use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use luna_bundle::lbp::{
    build_from_directory, BundleManifestInfo, Capabilities, EntryPoint, LbpArchive, LbpError,
    LbpManifest, MappingDeclaration, Metadata, PlatformInfo, FORMAT_VERSION, HEADER_SIZE,
    SECTION_ENTRY_SIZE,
};

fn temp_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("luna-lbp-integration-{stamp}"));
    fs::create_dir_all(&path).expect("create temporary fixture directory");
    path
}

fn manifest_with_mapping(source: &str, logical: &str) -> LbpManifest {
    LbpManifest {
        format: FORMAT_VERSION,
        bundle: BundleManifestInfo {
            id: "org.example.integration".into(),
            name: "Integration App".into(),
            version: "1.0.0".into(),
            kind: "application".into(),
        },
        platform: PlatformInfo {
            arch: "x86_64".into(),
            min_system: None,
        },
        entry: Some(EntryPoint {
            exec: "bin/app".into(),
            logical: Some("/usr/bin/app".into()),
        }),
        dependencies: Vec::new(),
        capabilities: Capabilities::default(),
        mappings: vec![MappingDeclaration {
            logical: logical.into(),
            source: source.into(),
            access: vec!["read".into()],
        }],
        metadata: Metadata::default(),
    }
}

#[test]
fn directory_mapping_preserves_nested_payload_files() {
    let root = temp_dir();
    fs::create_dir_all(root.join("resources/gtk/themes")).expect("create fixture tree");
    fs::create_dir_all(root.join("bin")).expect("create bin directory");
    fs::write(root.join("bin/app"), b"application").expect("write executable fixture");
    fs::write(root.join("resources/gtk/themes/default.ini"), b"theme").expect("write resource");

    let manifest = manifest_with_mapping("resources/gtk", "/usr/lib/gtk");
    let archive = build_from_directory(&manifest, &root).expect("build LBP1 archive");
    let parsed = LbpArchive::from_bytes(archive).expect("parse LBP1 archive");
    let payload = parsed.payload_bytes().expect("decode payload");

    let payload_text = String::from_utf8_lossy(&payload);
    assert!(payload_text.contains("resources/gtk/themes/default.ini"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_overlapping_sections() {
    let root = temp_dir();
    fs::create_dir_all(root.join("bin")).expect("create bin directory");
    fs::write(root.join("bin/app"), b"application").expect("write executable fixture");
    let manifest = manifest_with_mapping("bin/app", "/usr/bin/app-data");
    let mut bytes = build_from_directory(&manifest, &root).expect("build LBP1 archive");

    let first = HEADER_SIZE;
    let second = HEADER_SIZE + SECTION_ENTRY_SIZE;
    let first_offset = u64::from_le_bytes(bytes[first + 8..first + 16].try_into().unwrap());
    bytes[second + 8..second + 16].copy_from_slice(&first_offset.to_le_bytes());

    let mut normalized = [0u8; HEADER_SIZE];
    normalized.copy_from_slice(&bytes[..HEADER_SIZE]);
    normalized[32..64].fill(0);
    let digest = blake3::hash(&normalized);
    bytes[32..64].copy_from_slice(digest.as_bytes());

    assert!(matches!(
        LbpArchive::from_bytes(bytes),
        Err(LbpError::InvalidSectionTable)
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_absolute_and_traversal_sources() {
    let absolute = manifest_with_mapping("/etc/passwd", "/usr/bin/passwd");
    assert!(absolute.validate().is_err());

    let traversal = manifest_with_mapping("resources/../secret", "/usr/bin/secret");
    assert!(traversal.validate().is_err());
}

#[test]
fn rejects_duplicate_mapping_logical_paths() {
    let mut manifest = manifest_with_mapping("bin/app", "/usr/bin/app");
    manifest.mappings.push(MappingDeclaration {
        logical: "/usr/bin/app".into(),
        source: "bin/other".into(),
        access: vec!["execute".into()],
    });

    assert!(matches!(
        manifest.validate(),
        Err(LbpError::ManifestFormat(message)) if message.contains("duplicate mapping")
    ));
}
