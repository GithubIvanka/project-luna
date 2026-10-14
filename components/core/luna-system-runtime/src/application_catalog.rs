//! Ephemeral desktop presentation of a verified catalog, never an authority.
use std::collections::BTreeSet;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use luna_app_runtime::protocol::{CatalogResponse, DESKTOP_SHARE};

pub struct DesktopCatalog {
    directories: Vec<PathBuf>,
    files: BTreeSet<String>,
}

impl DesktopCatalog {
    pub fn new() -> io::Result<Self> {
        Self::at(
            Path::new(DESKTOP_SHARE)
                .parent()
                .expect("fixed runtime path"),
        )
    }

    fn at(root: &Path) -> io::Result<Self> {
        let parent = root
            .parent()
            .ok_or_else(|| io::Error::other("missing catalog parent"))?;
        let metadata = fs::symlink_metadata(parent)?;
        // The parent is already trusted runtime state, not a caller path.
        // SAFETY: geteuid has no preconditions and changes no state.
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o022 != 0
            || fs::canonicalize(parent)? != parent
        {
            return Err(io::Error::other("unsafe desktop catalog parent"));
        }
        let mut catalog = Self {
            directories: Vec::new(),
            files: BTreeSet::new(),
        };
        for path in [
            root.to_owned(),
            root.join("share"),
            root.join("share/applications"),
        ] {
            DirBuilder::new().mode(0o755).create(&path)?;
            catalog.directories.push(path.clone());
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
        Ok(catalog)
    }

    pub fn publish(&mut self, response: &CatalogResponse) -> io::Result<()> {
        response.to_toml()?;
        if response.error.is_some() {
            return Err(io::Error::other("failed catalog cannot be published"));
        }
        let entries = response
            .applications
            .iter()
            .map(|app| Ok((app.filename()?, app.desktop_entry()?)))
            .collect::<io::Result<Vec<_>>>()?;
        let desired = entries
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<BTreeSet<_>>();
        let directory = self
            .directories
            .last()
            .expect("owned application directory");
        for (name, text) in entries {
            let temporary = directory
                .parent()
                .expect("owned share directory")
                .join(format!(".{name}.new"));
            if fs::symlink_metadata(directory.join(&name)).is_ok() && !self.files.contains(&name) {
                return Err(io::Error::other("unexpected desktop catalog object"));
            }
            let mut created = false;
            let result = (|| {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o444)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(&temporary)?;
                created = true;
                file.set_permissions(fs::Permissions::from_mode(0o444))?;
                file.write_all(text.as_bytes())?;
                file.sync_all()?;
                fs::rename(&temporary, directory.join(&name))?;
                Ok::<_, io::Error>(())
            })();
            if created && result.is_err() {
                let _ = fs::remove_file(temporary);
            }
            result?;
            self.files.insert(name);
        }
        for name in self.files.difference(&desired).cloned().collect::<Vec<_>>() {
            fs::remove_file(directory.join(&name))?;
            self.files.remove(&name);
        }
        Ok(())
    }

    pub fn clear(&mut self) -> io::Result<()> {
        if let Some(directory) = self.directories.last() {
            for name in self.files.iter().cloned().collect::<Vec<_>>() {
                match fs::remove_file(directory.join(&name)) {
                    Ok(()) => {
                        self.files.remove(&name);
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        self.files.remove(&name);
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    }
}

impl Drop for DesktopCatalog {
    fn drop(&mut self) {
        let _ = self.clear();
        // Only files/directories created by this object are removed. Unknown
        // objects prevent rmdir; no recursive traversal of external entries.
        for directory in self.directories.iter().rev() {
            let _ = fs::remove_dir(directory);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luna_app_runtime::protocol::DesktopApplication;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "luna-desktop-catalog-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            DirBuilder::new().mode(0o700).create(&root).unwrap();
            Self(root)
        }
        fn catalog(&self) -> DesktopCatalog {
            DesktopCatalog::at(&self.0.join("catalog")).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn response(version: &str) -> CatalogResponse {
        CatalogResponse {
            format: 1,
            applications: vec![DesktopApplication {
                application: "org.luna.files".into(),
                version: version.into(),
                content_identity: "a".repeat(64),
                name: "Luna Files".into(),
                terminal: false,
            }],
            error: None,
        }
    }
    #[test]
    fn publication_reconciles_entries_and_keeps_directory_inode() {
        let fixture = Fixture::new();
        let mut catalog = fixture.catalog();
        let dir = fixture.0.join("catalog/share/applications");
        let inode = fs::metadata(&dir).unwrap().ino();
        let first = response("0.1.0");
        let name = first.applications[0].filename().unwrap();
        catalog.publish(&first).unwrap();
        assert_eq!(fs::metadata(dir.join(&name)).unwrap().mode() & 0o777, 0o444);
        let next = response("0.2.0");
        catalog.publish(&next).unwrap();
        assert!(!dir.join(name).exists());
        assert_eq!(fs::metadata(&dir).unwrap().ino(), inode);
        catalog.clear().unwrap();
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        drop(catalog);
        assert!(!fixture.0.join("catalog").exists());
    }
    #[test]
    fn unsafe_parent_and_existing_root_are_preserved() {
        let fixture = Fixture::new();
        let root = fixture.0.join("catalog");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("user-file"), b"keep").unwrap();
        assert!(DesktopCatalog::at(&root).is_err());
        assert_eq!(fs::read(root.join("user-file")).unwrap(), b"keep");
        fs::set_permissions(&fixture.0, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(DesktopCatalog::at(&fixture.0.join("other")).is_err());
    }
    #[test]
    fn foreign_temporary_and_final_files_are_not_removed() {
        let fixture = Fixture::new();
        let mut catalog = fixture.catalog();
        let response = response("0.1.0");
        let name = response.applications[0].filename().unwrap();
        let share = fixture.0.join("catalog/share");
        let tmp = share.join(format!(".{name}.new"));
        fs::write(&tmp, b"unrelated").unwrap();
        assert!(catalog.publish(&response).is_err());
        assert_eq!(fs::read(&tmp).unwrap(), b"unrelated");
        fs::remove_file(tmp).unwrap();
        let final_path = share.join("applications").join(name);
        fs::write(&final_path, b"foreign").unwrap();
        assert!(catalog.publish(&response).is_err());
        drop(catalog);
        assert_eq!(fs::read(final_path).unwrap(), b"foreign");
    }
    #[test]
    fn symlink_parent_never_changes_external_contents() {
        let fixture = Fixture::new();
        let outside = fixture.0.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep"), b"safe").unwrap();
        std::os::unix::fs::symlink(&outside, fixture.0.join("alias")).unwrap();
        assert!(DesktopCatalog::at(&fixture.0.join("alias/catalog")).is_err());
        assert_eq!(fs::read(outside.join("keep")).unwrap(), b"safe");
    }
}
