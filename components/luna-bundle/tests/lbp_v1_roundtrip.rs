use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use luna_bundle::lbp::{
    build_from_directory, BundleManifestInfo, Capabilities, EntryPoint, LbpArchive, LbpError,
    LbpManifest, MappingDeclaration, Metadata, PlatformInfo, FORMAT_VERSION,
};

fn temp_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("luna-lbp-roundtrip-{stamp}"));
    fs::create_dir_all(&path).expect("create temporary fixture directory");
    path
}

fn manifest() -> LbpManifest {
    LbpManifest {
        format: FORMAT_VERSION,
        bundle: BundleManifestInfo {
            id: "org.example.roundtrip".into(),
            name: "Roundtrip App".into(),
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
            logical: "/usr/bin/app".into(),
            source: "bin/app".into(),
            access: vec!["execute".into()],
        }],
        metadata: Metadata::default(),
    }
}

fn fixture(root: &Path) {
    fs::create_dir_all(root.join("bin")).expect("create bin directory");
    fs::write(root.join("bin/app"), b"#!/bin/sh\necho luna\n").expect("write executable fixture");
}

#[test]
fn build_is_deterministic_for_identical_inputs() {
    let root = temp_dir();
    fixture(&root);
    let manifest = manifest();

    let first = build_from_directory(&manifest, &root).expect("build first archive");
    let second = build_from_directory(&manifest, &root).expect("build second archive");

    assert_eq!(first, second);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn roundtrip_preserves_manifest_and_payload() {
    let root = temp_dir();
    fixture(&root);
    let manifest = manifest();

    let bytes = build_from_directory(&manifest, &root).expect("build archive");
    let archive = LbpArchive::from_bytes(bytes).expect("parse archive");

    assert_eq!(archive.manifest, manifest);
    let payload = archive.payload_bytes().expect("read payload");
    assert!(!payload.is_empty());

    let destination = temp_dir();
    archive
        .extract_payload(&destination)
        .expect("extract payload");
    assert_eq!(
        fs::read(destination.join("bin/app")).unwrap(),
        b"#!/bin/sh\necho luna\n"
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(destination);
}

#[test]
fn corrupting_payload_is_rejected() {
    let root = temp_dir();
    fixture(&root);
    let manifest = manifest();
    let mut bytes = build_from_directory(&manifest, &root).expect("build archive");

    let archive = LbpArchive::from_bytes(bytes.clone()).expect("parse original archive");
    let payload = archive
        .sections
        .iter()
        .find(|section| matches!(section.kind, luna_bundle::lbp::SectionKind::Payload))
        .expect("payload section");
    let payload_offset = usize::try_from(payload.offset).expect("payload offset fits usize");
    assert!(payload.compressed_length > 0);
    bytes[payload_offset] ^= 0x01;

    assert!(matches!(
        LbpArchive::from_bytes(bytes),
        Err(LbpError::HashMismatch)
    ));

    let _ = fs::remove_dir_all(root);
}
