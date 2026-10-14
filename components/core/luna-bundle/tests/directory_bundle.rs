use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use luna_bundle::directory::{
    DirectoryBundle, application_filename, materialize_archive, storage_slot,
};
use luna_bundle::lbp::{LbpArchive, LbpManifest, build_from_directory};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "luna-directory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn archive(&self) -> LbpArchive {
        let payload = self.0.join("source");
        fs::create_dir(&payload).unwrap();
        fs::create_dir(payload.join("bin")).unwrap();
        fs::write(payload.join("bin/cli"), b"native executable fixture").unwrap();
        mode(&payload.join("bin/cli"), 0o755);
        let manifest = LbpManifest::from_toml(
            r#"
format = 1
[bundle]
id = "org.example.cli"
name = "Luna CLI"
version = "1.0.0"
type = "application"
[platform]
arch = "x86_64"
[entry]
exec = "bin/cli"
"#,
        )
        .unwrap();
        LbpArchive::from_bytes(build_from_directory(&manifest, payload).unwrap()).unwrap()
    }
    fn package(&self) -> (PathBuf, [u8; 32]) {
        let archive = self.archive();
        let package = self.0.join(application_filename(&archive.manifest));
        fs::create_dir(&package).unwrap();
        materialize_archive(&archive, &package).unwrap();
        freeze(&package);
        (package, archive.content_identity().unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fn remove(path: &Path) {
            let metadata = fs::symlink_metadata(path).unwrap();
            if metadata.is_dir() {
                mode(path, 0o700);
                for entry in fs::read_dir(path).unwrap() {
                    remove(&entry.unwrap().path());
                }
                fs::remove_dir(path).unwrap();
            } else {
                fs::remove_file(path).unwrap();
            }
        }
        remove(&self.0);
    }
}
fn mode(path: &Path, value: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(value)).unwrap();
}
fn freeze(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    if metadata.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            freeze(&entry.unwrap().path());
        }
        mode(path, 0o555);
    } else {
        mode(
            path,
            if metadata.permissions().mode() & 0o111 != 0 {
                0o555
            } else {
                0o444
            },
        );
    }
}

#[test]
fn directory_identity_survives_transport_absence_and_rename() {
    let f = Fixture::new();
    let (package, identity) = f.package();
    assert_eq!(package.file_name().unwrap(), "luna-cli.lunaapp");
    assert!(!package.join("bundle.lbp").exists());
    assert_eq!(
        DirectoryBundle::read(&package).unwrap().content_identity,
        identity
    );
    mode(&package, 0o755);
    let renamed = f.0.join("Renamed application");
    fs::rename(&package, &renamed).unwrap();
    mode(&renamed, 0o555);
    assert_eq!(
        DirectoryBundle::read(renamed).unwrap().content_identity,
        identity
    );
}

#[test]
fn names_do_not_encode_id_or_version() {
    let f = Fixture::new();
    let archive = f.archive();
    let mut next = archive.manifest.clone();
    next.bundle.id = "another.identity".into();
    next.bundle.version = "9.0.0".into();
    assert_eq!(
        application_filename(&archive.manifest),
        application_filename(&next)
    );
    assert_ne!(
        storage_slot("another.identity", "1.0.0"),
        storage_slot("another.identity", "9.0.0")
    );
}

#[test]
fn extension_alone_and_ancestor_links_are_not_valid_bundles() {
    let f = Fixture::new();
    let (package, _) = f.package();
    let fake = f.0.join("fake.lunaapp");
    fs::create_dir(&fake).unwrap();
    mode(&fake, 0o555);
    assert!(DirectoryBundle::read(fake).is_err());
    let alias = f.0.join("alias");
    symlink(&package, &alias).unwrap();
    assert!(DirectoryBundle::read(&alias).is_err());
    let parent = f.0.join("parent-alias");
    symlink(&f.0, &parent).unwrap();
    assert!(DirectoryBundle::read(parent.join(package.file_name().unwrap())).is_err());
}

#[test]
fn changed_bytes_modes_and_hardlinks_are_rejected() {
    let f = Fixture::new();
    let (package, _) = f.package();
    let file = package.join("payload/bin/cli");
    mode(&file, 0o755);
    fs::write(&file, b"modified").unwrap();
    mode(&file, 0o555);
    assert!(DirectoryBundle::read(&package).is_err());
    mode(&file, 0o755);
    fs::write(&file, b"native executable fixture").unwrap();
    mode(&file, 0o555);
    assert!(DirectoryBundle::read(&package).is_ok());
    fs::hard_link(&file, f.0.join("hardlink")).unwrap();
    assert!(DirectoryBundle::read(&package).is_err());
    fs::remove_file(f.0.join("hardlink")).unwrap();
    mode(&file, 0o444);
    assert!(DirectoryBundle::read(&package).is_err());
}

#[test]
fn extras_and_embedded_transport_are_rejected() {
    let f = Fixture::new();
    let (package, _) = f.package();
    mode(&package, 0o755);
    fs::write(package.join("bundle.lbp"), b"transport").unwrap();
    mode(&package, 0o555);
    assert!(DirectoryBundle::read(&package).is_err());
    mode(&package, 0o755);
    fs::remove_file(package.join("bundle.lbp")).unwrap();
    mode(&package, 0o555);
    let payload = package.join("payload");
    mode(&payload, 0o755);
    fs::create_dir(payload.join("extra")).unwrap();
    mode(&payload.join("extra"), 0o555);
    mode(&payload, 0o555);
    assert!(DirectoryBundle::read(package).is_err());
}

#[test]
fn fifo_and_payload_symlinks_fail_without_reading_them() {
    let f = Fixture::new();
    let (package, _) = f.package();
    let bin = package.join("payload/bin");
    let file = bin.join("cli");
    mode(&bin, 0o755);
    fs::remove_file(&file).unwrap();
    symlink("/dev/zero", &file).unwrap();
    mode(&bin, 0o555);
    assert!(DirectoryBundle::read(&package).is_err());
    mode(&bin, 0o755);
    fs::remove_file(&file).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(file.as_os_str().as_bytes()).unwrap();
    // SAFETY: valid NUL-terminated fixture path; creates only a temporary FIFO.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o444) }, 0);
    mode(&bin, 0o555);
    assert!(DirectoryBundle::read(package).is_err());
}

#[test]
fn oversized_sparse_payload_fails_before_hashing() {
    let f = Fixture::new();
    let (package, _) = f.package();
    let file = package.join("payload/bin/cli");
    mode(&file, 0o755);
    fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap()
        .set_len((1 << 30) + 1)
        .unwrap();
    mode(&file, 0o555);
    assert!(DirectoryBundle::read(package).is_err());
}

// Construct valid container variants independent of the normal writer, to
// distinguish valid LBP transport from supported directory-v1 representation.
fn container(sections: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let end = 64 + sections.len() * 64;
    let mut bytes = vec![0u8; end];
    bytes[..4].copy_from_slice(b"LBP1");
    bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
    bytes[8..12].copy_from_slice(&(sections.len() as u32).to_le_bytes());
    bytes[12..20].copy_from_slice(&64u64.to_le_bytes());
    bytes[20..28].copy_from_slice(&((sections.len() * 64) as u64).to_le_bytes());
    bytes[28..32].copy_from_slice(&64u32.to_le_bytes());
    for (index, (kind, data)) in sections.iter().enumerate() {
        let entry = 64 + index * 64;
        let offset = bytes.len() as u64;
        bytes[entry..entry + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[entry + 8..entry + 16].copy_from_slice(&offset.to_le_bytes());
        bytes[entry + 16..entry + 24].copy_from_slice(&(data.len() as u64).to_le_bytes());
        bytes[entry + 24..entry + 32].copy_from_slice(&(data.len() as u64).to_le_bytes());
        bytes[entry + 32..entry + 64].copy_from_slice(blake3::hash(data).as_bytes());
        bytes.extend_from_slice(data);
    }
    let hash = blake3::hash(&bytes[..64]);
    bytes[32..64].copy_from_slice(hash.as_bytes());
    bytes
}

#[test]
fn unsupported_sections_are_not_silently_dropped() {
    let f = Fixture::new();
    let archive = f.archive();
    for kind in [3, 4] {
        let variant = LbpArchive::from_bytes(container(&[
            (1, archive.manifest_bytes().unwrap()),
            (2, archive.payload_bytes().unwrap()),
            (kind, b"unverified section".to_vec()),
        ]))
        .unwrap();
        let root = f.0.join(format!("variant-{kind}"));
        fs::create_dir(&root).unwrap();
        assert!(materialize_archive(&variant, &root).is_err());
        assert_eq!(fs::read_dir(root).unwrap().count(), 0);
    }
}

#[test]
fn noncanonical_tar_retains_identity_and_is_refused() {
    let f = Fixture::new();
    let archive = f.archive();
    let mut payload = archive.payload_bytes().unwrap();
    let mut header = tar::Header::from_byte_slice(&payload[..512]).clone();
    header.set_uid(42);
    header.set_cksum();
    payload[..512].copy_from_slice(header.as_bytes());
    let variant = LbpArchive::from_bytes(container(&[
        (1, archive.manifest_bytes().unwrap()),
        (2, payload),
    ]))
    .unwrap();
    let identity = variant.content_identity().unwrap();
    assert_ne!(identity, archive.content_identity().unwrap());
    let root = f.0.join("noncanonical");
    fs::create_dir(&root).unwrap();
    assert!(materialize_archive(&variant, &root).is_err());
    assert_eq!(variant.content_identity().unwrap(), identity);
}
