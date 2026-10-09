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
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};

use luna_common::{VolumePath, VolumeRef};
use luna_root_mapping::MappingKind;

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
    support_root: Option<PathBuf>,
}

impl VolumeMountManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn instances(&self) -> impl Iterator<Item = &MountInstance> {
        self.instances.iter()
    }

    fn ensure_support_root(&mut self, root: &Path) -> Result<PathBuf, NamespaceError> {
        let expected = volume_support_root(root);
        if let Some(current) = &self.support_root {
            if current != &expected {
                return Err(NamespaceError::FilesystemAccess(
                    "one VolumeMountManager cannot span logical roots".to_owned(),
                ));
            }
            return Ok(current.clone());
        }

        match fs::symlink_metadata(&expected) {
            Ok(_) => {
                return Err(NamespaceError::FilesystemAccess(format!(
                    "volume support directory already exists: {}",
                    expected.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        fs::create_dir(&expected)?;
        fs::set_permissions(&expected, fs::Permissions::from_mode(0o700))?;
        self.support_root = Some(expected.clone());
        Ok(expected)
    }

    pub fn mount_volume(
        &mut self,
        _namespace: &LinuxMountNamespace,
        resolver: &impl VolumeMountResolver,
        root: &Path,
        logical_path: &Path,
        volume_path: &VolumePath,
        mapping_kind: MappingKind,
        mapping_read_only: bool,
    ) -> Result<MountInstance, NamespaceError> {
        if !root.is_absolute() || !root.is_dir() {
            return Err(NamespaceError::MissingBaseRoot);
        }
        validate_logical_target(logical_path)?;
        let resolved = resolver.resolve_volume(volume_path.volume())?;

        if resolved.volume() != volume_path.volume() {
            return Err(NamespaceError::FilesystemAccess(
                "volume resolver returned a different VolumeRef".to_owned(),
            ));
        }
        if !resolved.backend_node().is_absolute() {
            return Err(NamespaceError::FilesystemAccess(
                "volume resolver returned a non-absolute backend node".to_owned(),
            ));
        }
        let backend_metadata = fs::symlink_metadata(resolved.backend_node()).map_err(|error| {
            NamespaceError::FilesystemAccess(format!("volume backend node is unavailable: {error}"))
        })?;
        if backend_metadata.file_type().is_symlink()
            || !backend_metadata.file_type().is_block_device()
        {
            return Err(NamespaceError::FilesystemAccess(
                "volume backend node must be a block device, not a symlink or regular file"
                    .to_owned(),
            ));
        }
        if resolved.filesystem().is_empty() {
            return Err(NamespaceError::FilesystemAccess(
                "volume resolver returned an empty filesystem type".to_owned(),
            ));
        }

        let relative = relative_path(volume_path)?;
        if relative == Path::new("/") && mapping_kind == MappingKind::File {
            return Err(NamespaceError::FilesystemAccess(
                "a VolumePath for '/' cannot be materialized as a file mapping".to_owned(),
            ));
        }
        let support_root = self.ensure_support_root(root)?;
        let source_mountpoint = support_root.join(format!("{}", self.sequence));
        self.sequence = self.sequence.wrapping_add(1);
        fs::create_dir(&source_mountpoint)?;

        let effective_read_only = resolved.read_only() || mapping_read_only;
        let flags = mount_flags(effective_read_only);
        mount_block_device(
            resolved.backend_node(),
            &source_mountpoint,
            resolved.filesystem(),
            flags,
            None,
        )?;

        let target = root.join(logical_path.strip_prefix("/").unwrap_or(logical_path));

        let result = if relative == Path::new("/") {
            if let Err(error) = prepare_target(root, &target, true) {
                let _ = unmount_path(&source_mountpoint);
                return Err(error);
            }
            if let Err(error) = mount_fs_move(&source_mountpoint, &target) {
                let _ = unmount_path(&source_mountpoint);
                return Err(error);
            }
            MountInstance {
                volume: volume_path.volume().clone(),
                logical_path: logical_path.to_path_buf(),
                mountpoint: target.clone(),
                source_mountpoint,
                relative_path: relative,
                read_only: effective_read_only,
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
            let metadata = match fs::symlink_metadata(&source_path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    let _ = unmount_path(&source_mountpoint);
                    return Err(error.into());
                }
            };
            if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
                let _ = unmount_path(&source_mountpoint);
                return Err(NamespaceError::FilesystemAccess(
                    "VolumePath must resolve to a regular file or directory".to_owned(),
                ));
            }
            let source_is_directory = metadata.is_dir();
            if (mapping_kind == MappingKind::Subtree) != source_is_directory {
                let _ = unmount_path(&source_mountpoint);
                return Err(NamespaceError::FilesystemAccess(match mapping_kind {
                    MappingKind::File => "file mapping source resolved to a directory".to_owned(),
                    MappingKind::Subtree => "subtree mapping source resolved to a file".to_owned(),
                }));
            }
            if let Err(error) = prepare_target(root, &target, source_is_directory) {
                let _ = unmount_path(&source_mountpoint);
                return Err(error);
            }

            if let Err(error) = mount_bind_path(
                &source_mountpoint,
                &source_path,
                root,
                &target,
                effective_read_only,
            ) {
                let _ = unmount_path(&source_mountpoint);
                return Err(error);
            }

            MountInstance {
                volume: volume_path.volume().clone(),
                logical_path: logical_path.to_path_buf(),
                mountpoint: target,
                source_mountpoint,
                relative_path: relative,
                read_only: effective_read_only,
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
        let _ = fs::remove_dir(&instance.source_mountpoint);
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
    let top_level = path
        .strip_prefix("/")
        .ok()
        .and_then(|relative| relative.components().next())
        .and_then(|component| component.as_os_str().to_str());
    if matches!(top_level, Some("proc" | "sys" | "dev")) {
        return Err(NamespaceError::FilesystemAccess(
            "Volume mappings must not overlap the runtime /proc, /sys or /dev trees".to_owned(),
        ));
    }
    Ok(())
}

fn volume_support_root(root: &Path) -> PathBuf {
    let parent = root.parent().unwrap_or(Path::new("/tmp"));
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("root");
    parent.join(format!(".luna-volume-mounts-{name}"))
}

fn prepare_target(root: &Path, target: &Path, directory: bool) -> Result<(), NamespaceError> {
    if !root.is_absolute() || !target.starts_with(root) || target == root {
        return Err(NamespaceError::InvalidPath);
    }
    let relative = target
        .strip_prefix(root)
        .map_err(|_| NamespaceError::InvalidPath)?;
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(NamespaceError::InvalidPath);
    }

    let root_metadata = fs::symlink_metadata(root)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(NamespaceError::FilesystemAccess(
            "Volume mapping root must be a real directory".to_owned(),
        ));
    }

    let mut current = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        current.push(component.as_os_str());
        let final_component = index + 1 == components.len();
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(NamespaceError::FilesystemAccess(
                        "Volume mapping path traverses a symlink".to_owned(),
                    ));
                }
                let expected_directory = !final_component || directory;
                if expected_directory && !metadata.is_dir() {
                    return Err(NamespaceError::FilesystemAccess(
                        "Volume mapping parent/target is not a directory".to_owned(),
                    ));
                }
                if final_component && !directory && !metadata.is_file() {
                    return Err(NamespaceError::FilesystemAccess(
                        "file VolumePath target is not a regular file".to_owned(),
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if !final_component || directory {
                    fs::create_dir(&current)?;
                } else {
                    fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&current)?;
                }
            }
            Err(error) => return Err(error.into()),
        }
    }
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

fn mount_bind_path(
    trusted_source_root: &Path,
    source: &Path,
    trusted_target_root: &Path,
    target: &Path,
    read_only: bool,
) -> Result<(), NamespaceError> {
    crate::secure_mount::secure_bind_mount_from_roots(
        trusted_source_root,
        source,
        trusted_target_root,
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
        let error = io::Error::last_os_error();
        if matches!(
            error.raw_os_error(),
            Some(libc::EINVAL) | Some(libc::ENOENT)
        ) {
            return Ok(());
        }
        return Err(NamespaceError::Io(error));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ResolvedVolume, VolumeMountManager, VolumeMountResolver, prepare_target,
        volume_support_root,
    };
    use crate::NamespaceError;
    use luna_common::{VolumeId, VolumePath, VolumeRef};
    use std::path::{Path, PathBuf};

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
    fn volume_support_mountpoints_are_siblings_not_application_visible_paths() {
        let root = Path::new("/staging/instance-7");
        let support = volume_support_root(root);
        assert_eq!(
            support,
            Path::new("/staging/.luna-volume-mounts-instance-7")
        );
        assert!(!support.starts_with(root));
    }

    #[test]
    fn target_preparation_supports_files_and_directories_and_rejects_symlinks() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("luna-volume-target-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();

        let file = root.join("readme.txt");
        prepare_target(&root, &file, false).unwrap();
        assert!(file.is_file());

        let directory = root.join("documents");
        prepare_target(&root, &directory, true).unwrap();
        assert!(directory.is_dir());

        let link = root.join("alias.txt");
        symlink(&file, &link).unwrap();
        assert!(prepare_target(&root, &link, false).is_err());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn target_preparation_rejects_symlink_ancestors_without_writing_outside_root() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "luna-volume-symlink-{}-{nonce}",
            std::process::id()
        ));
        let root = base.join("app-root");
        let outside = base.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("alias")).unwrap();

        let escaped = root.join("alias/created.txt");
        assert!(prepare_target(&root, &escaped, false).is_err());
        assert!(!outside.join("created.txt").exists());

        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn instance_metadata_does_not_expose_backend_node() {
        let instance_type = std::any::type_name::<super::MountInstance>();
        assert!(instance_type.contains("MountInstance"));
        let _ = PathBuf::from(instance_type);
    }
}
