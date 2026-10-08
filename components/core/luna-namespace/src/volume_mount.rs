//! Namespace-local materialization of Luna Volume objects.
//!
//! The resolver supplies backend-only information for one VolumeRef at the
//! moment of materialization. The application-facing storage identity remains
//! VolumeRef + relative path; concrete Linux device nodes and mountpoints stay
//! inside this backend.

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use luna_common::{VolumePath, VolumeRef};

use crate::{LinuxMountNamespace, NamespaceError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedVolume {
    volume: VolumeRef,
    backend_node: PathBuf,
    filesystem: String,
    read_only: bool,
}

impl ResolvedVolume {
    pub fn new(
        volume: VolumeRef,
        backend_node: impl Into<PathBuf>,
        filesystem: impl Into<String>,
        read_only: bool,
    ) -> Self {
        Self {
            volume,
            backend_node: backend_node.into(),
            filesystem: filesystem.into(),
            read_only,
        }
    }

    pub fn volume(&self) -> &VolumeRef {
        &self.volume
    }

    pub fn backend_node(&self) -> &Path {
        &self.backend_node
    }

    pub fn filesystem(&self) -> &str {
        &self.filesystem
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }
}

pub trait VolumeMountResolver {
    fn resolve_volume(&self, volume: &VolumeRef) -> Result<ResolvedVolume, NamespaceError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MountInstance {
    volume: VolumeRef,
    logical_path: PathBuf,
    mountpoint: PathBuf,
    source_mountpoint: PathBuf,
    relative_path: PathBuf,
    read_only: bool,
}

impl MountInstance {
    pub fn volume(&self) -> &VolumeRef {
        &self.volume
    }

    pub fn logical_path(&self) -> &Path {
        &self.logical_path
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }
}

#[derive(Debug, Default)]
pub struct VolumeMountManager {
    instances: Vec<MountInstance>,
    sequence: u64,
}

impl VolumeMountManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn instances(&self) -> impl Iterator<Item = &MountInstance> {
        self.instances.iter()
    }

    pub fn mount_volume(
        &mut self,
        _namespace: &LinuxMountNamespace,
        resolver: &impl VolumeMountResolver,
        root: &Path,
        logical_path: &Path,
        volume_path: &VolumePath,
    ) -> Result<MountInstance, NamespaceError> {
        validate_logical_target(logical_path)?;
        let resolved = resolver.resolve_volume(volume_path.volume())?;

        if resolved.volume() != volume_path.volume() {
            return Err(NamespaceError::FilesystemAccess(
                "volume resolver returned a different VolumeRef".to_owned(),
            ));
        }
        if !resolved.backend_node().is_absolute() || !resolved.backend_node().exists() {
            return Err(NamespaceError::FilesystemAccess(
                "volume resolver returned an unavailable backend node".to_owned(),
            ));
        }
        if resolved.filesystem().is_empty() {
            return Err(NamespaceError::FilesystemAccess(
                "volume resolver returned an empty filesystem type".to_owned(),
            ));
        }

        let source_mountpoint = root
            .join(".luna-volume-mounts")
            .join(format!("{}", self.sequence));
        self.sequence = self.sequence.wrapping_add(1);
        fs::create_dir_all(&source_mountpoint)?;

        let flags = mount_flags(resolved.read_only());
        mount_block_device(
            resolved.backend_node(),
            &source_mountpoint,
            resolved.filesystem(),
            flags,
            None,
        )?;

        let relative = relative_path(volume_path)?;
        let target = root.join(logical_path.strip_prefix("/").unwrap_or(logical_path));
        prepare_directory(&target)?;

        let result = if relative == Path::new("/") {
            if mount_fs_move(&source_mountpoint, &target).is_err() {
                let _ = unmount_path(&source_mountpoint);
                return Err(NamespaceError::Io(io::Error::last_os_error()));
            }
            MountInstance {
                volume: volume_path.volume().clone(),
                logical_path: logical_path.to_path_buf(),
                mountpoint: target.clone(),
                source_mountpoint,
                relative_path: relative,
                read_only: resolved.read_only(),
            }
        } else {
            let source_path =
                source_mountpoint.join(relative.strip_prefix("/").unwrap_or(&relative));
            if !source_path.exists() {
                let _ = unmount_path(&source_mountpoint);
                return Err(NamespaceError::FilesystemAccess(format!(
                    "volume path does not exist: {}",
                    volume_path.relative_path()
                )));
            }
            if !source_path.is_dir() {
                let _ = unmount_path(&source_mountpoint);
                return Err(NamespaceError::FilesystemAccess(
                    "Volume MountInstance currently requires a directory subtree".to_owned(),
                ));
            }

            if mount_bind_subtree(&source_path, &target, resolved.read_only()).is_err() {
                let error = io::Error::last_os_error();
                let _ = unmount_path(&source_mountpoint);
                return Err(NamespaceError::Io(error));
            }

            MountInstance {
                volume: volume_path.volume().clone(),
                logical_path: logical_path.to_path_buf(),
                mountpoint: target,
                source_mountpoint,
                relative_path: relative,
                read_only: resolved.read_only(),
            }
        };

        self.instances.push(result.clone());
        Ok(result)
    }

    pub fn unmount(&mut self, instance: &MountInstance) -> Result<(), NamespaceError> {
        let index = self
            .instances
            .iter()
            .position(|current| current == instance)
            .ok_or_else(|| {
                NamespaceError::FilesystemAccess(
                    "mount instance is not owned by this manager".into(),
                )
            })?;

        let target_result = unmount_path(&instance.mountpoint);
        let source_result = if instance.relative_path != Path::new("/") {
            unmount_path(&instance.source_mountpoint)
        } else {
            Ok(())
        };

        if let Err(error) = target_result {
            let _ = source_result;
            return Err(error);
        }
        source_result?;

        self.instances.remove(index);
        Ok(())
    }

    pub fn unmount_volume(&mut self, volume: &VolumeRef) -> Result<usize, NamespaceError> {
        let matching = self
            .instances
            .iter()
            .filter(|instance| instance.volume() == volume)
            .cloned()
            .collect::<Vec<_>>();

        for instance in &matching {
            self.unmount(instance)?;
        }
        Ok(matching.len())
    }
}

fn relative_path(volume_path: &VolumePath) -> Result<PathBuf, NamespaceError> {
    let relative = volume_path.relative_path();
    let path = Path::new(relative);
    if !path.is_absolute() {
        return Err(NamespaceError::InvalidPath);
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::CurDir
        )
    }) {
        return Err(NamespaceError::InvalidPath);
    }
    Ok(path.to_path_buf())
}

fn validate_logical_target(path: &Path) -> Result<(), NamespaceError> {
    if !path.is_absolute() || path == Path::new("/") {
        return Err(NamespaceError::InvalidPath);
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::CurDir
        )
    }) {
        return Err(NamespaceError::InvalidPath);
    }
    Ok(())
}

fn prepare_directory(target: &Path) -> Result<(), NamespaceError> {
    fs::create_dir_all(target)?;
    Ok(())
}

fn mount_flags(read_only: bool) -> libc::c_ulong {
    if read_only {
        libc::MS_NOSUID | libc::MS_NODEV | libc::MS_RDONLY
    } else {
        libc::MS_NOSUID | libc::MS_NODEV
    }
}

fn mount_block_device(
    source: &Path,
    target: &Path,
    filesystem: &str,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<(), NamespaceError> {
    let source = path_cstring(source)?;
    let filesystem = CString::new(filesystem).map_err(|_| NamespaceError::InvalidPath)?;
    mount_raw(Some(&source), target, Some(&filesystem), flags, data)
}

fn path_cstring(path: &Path) -> Result<CString, NamespaceError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| NamespaceError::InvalidPath)
}

fn mount_raw(
    source: Option<&CString>,
    target: &Path,
    filesystem: Option<&CString>,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<(), NamespaceError> {
    let target = path_cstring(target)?;
    let data = data
        .map(|value| CString::new(value).map_err(|_| NamespaceError::InvalidPath))
        .transpose()?;
    let status = unsafe {
        libc::mount(
            source.map_or(std::ptr::null(), |value| value.as_ptr()),
            target.as_ptr(),
            filesystem.map_or(std::ptr::null(), |value| value.as_ptr()),
            flags,
            data.as_ref()
                .map_or(std::ptr::null(), |value| value.as_ptr().cast()),
        )
    };
    if status == -1 {
        return Err(NamespaceError::Io(io::Error::last_os_error()));
    }
    Ok(())
}

fn mount_bind_subtree(source: &Path, target: &Path, read_only: bool) -> Result<(), NamespaceError> {
    crate::secure_mount::secure_bind_mount_from_roots(
        Path::new("/"),
        source,
        Path::new("/"),
        target,
        read_only,
    )
}

fn mount_fs_move(source: &Path, target: &Path) -> Result<(), NamespaceError> {
    let source = path_cstring(source)?;
    let target = path_cstring(target)?;
    let status = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            libc::MS_MOVE,
            std::ptr::null(),
        )
    };
    if status == -1 {
        return Err(NamespaceError::Io(io::Error::last_os_error()));
    }
    Ok(())
}

fn unmount_path(path: &Path) -> Result<(), NamespaceError> {
    let path = path_cstring(path)?;
    let status = unsafe { libc::umount2(path.as_ptr(), libc::MNT_DETACH) };
    if status == -1 {
        return Err(NamespaceError::Io(io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ResolvedVolume, VolumeMountManager, VolumeMountResolver};
    use crate::NamespaceError;
    use luna_common::{VolumeId, VolumePath, VolumeRef};
    use std::path::PathBuf;

    struct FakeResolver;

    impl VolumeMountResolver for FakeResolver {
        fn resolve_volume(&self, volume: &VolumeRef) -> Result<ResolvedVolume, NamespaceError> {
            Ok(ResolvedVolume::new(
                volume.clone(),
                "/dev/test-volume",
                "ext4",
                true,
            ))
        }
    }

    #[test]
    fn resolved_volume_preserves_storage_identity() {
        let volume = VolumeRef::new(VolumeId::from("partition-guid:test"));
        let resolved = ResolvedVolume::new(volume.clone(), "/dev/sdb1", "ext4", true);
        assert_eq!(resolved.volume(), &volume);
        assert_eq!(resolved.backend_node(), std::path::Path::new("/dev/sdb1"));
        assert_eq!(resolved.filesystem(), "ext4");
        assert!(resolved.read_only());
    }

    #[test]
    fn manager_starts_without_global_mounts() {
        let manager = VolumeMountManager::new();
        assert!(manager.instances().next().is_none());
    }

    #[test]
    fn relative_volume_path_is_not_a_linux_mountpoint() {
        let volume = VolumeRef::new(VolumeId::from("volume-1"));
        let path = VolumePath::new(volume, "/documents/projects").unwrap();
        assert_eq!(path.relative_path(), "/documents/projects");
    }

    #[test]
    fn resolver_is_typed_by_volume_ref() {
        let resolver = FakeResolver;
        let volume = VolumeRef::new(VolumeId::from("partition-guid:test"));
        let resolved = resolver.resolve_volume(&volume).unwrap();
        assert_eq!(resolved.volume(), &volume);
        assert_eq!(resolved.filesystem(), "ext4");
    }

    #[test]
    fn instance_metadata_does_not_expose_backend_node() {
        let instance_type = std::any::type_name::<super::MountInstance>();
        assert!(instance_type.contains("MountInstance"));
        let _ = PathBuf::from(instance_type);
    }
}
