use luna_app_manager::{FilesystemApplicationManager, inspect_bundle};
use luna_bundle::lbp::{LbpManifest, build_from_directory};
use luna_common::Version;
use luna_security::{BundleTrustScope, Permission, Principal, Resource, StaticPolicyAuthority};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "luna-desktop-inventory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        Self(root)
    }
    fn install(&self, manager: &FilesystemApplicationManager, id: &str, version: &str) {
        let source = self.0.join(format!("source-{id}-{version}"));
        fs::create_dir(&source).unwrap();
        fs::create_dir(source.join("bin")).unwrap();
        fs::write(source.join("bin/app"), b"verified payload").unwrap();
        fs::set_permissions(source.join("bin/app"), fs::Permissions::from_mode(0o755)).unwrap();
        let manifest=LbpManifest::from_toml(&format!("format=1\n[bundle]\nid='{id}'\nname='Friendly Application'\nversion='{version}'\ntype='application'\n[platform]\narch='x86_64'\n[entry]\nexec='bin/app'\nlogical='/bin/app'\n[[mapping]]\nlogical='/bin/app'\nsource='bin/app'\naccess=['read','execute']\n")).unwrap();
        let archive = self.0.join(format!("{id}-{version}.lbp"));
        fs::write(&archive, build_from_directory(&manifest, &source).unwrap()).unwrap();
        let info = inspect_bundle(&archive).unwrap();
        let mut policy = StaticPolicyAuthority::new();
        policy.grant(
            Principal::System,
            Resource::Application(info.application.id().clone()),
            Permission::Manage,
        );
        policy.grant(
            Principal::System,
            Resource::BundleTrust {
                application: info.application.id().clone(),
                content_identity: info.content_identity,
                scope: BundleTrustScope::Install,
            },
            Permission::Use,
        );
        let mut state = manager.open_offline_state().unwrap();
        manager.install(archive, &policy, &mut state).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fn thaw(path: &std::path::Path) {
            if path.is_dir() {
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
                for entry in fs::read_dir(path).unwrap() {
                    thaw(&entry.unwrap().path());
                }
            }
        }
        thaw(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn inventory_is_registered_verified_sorted_and_bounded() {
    let fixture = Fixture::new();
    let data = fixture.0.join("data");
    fs::create_dir(&data).unwrap();
    fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).unwrap();
    let manager = FilesystemApplicationManager::new(&data, Version::new(0, 1, 0)).unwrap();
    fixture.install(&manager, "org.test.b", "0.1.0");
    fixture.install(&manager, "org.test.a", "0.2.0");
    let entries = manager.list_bounded(2).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].inspection.application.id().as_str(),
        "org.test.a"
    );
    assert_eq!(
        entries[0].inspection.manifest.bundle.name,
        "Friendly Application"
    );
    assert!(manager.list_bounded(1).is_err());
    assert!(manager.list_bounded(0).is_err());
    let payload = entries[0].payload_root.join("bin/app");
    fs::set_permissions(&payload, fs::Permissions::from_mode(0o644)).unwrap();
    fs::write(&payload, b"changed payload").unwrap();
    fs::set_permissions(payload, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(manager.list_bounded(2).is_err());
}

#[test]
fn a_directory_suffix_alone_is_not_a_registration() {
    let fixture = Fixture::new();
    let data = fixture.0.join("data");
    fs::create_dir(&data).unwrap();
    fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).unwrap();
    let manager = FilesystemApplicationManager::new(&data, Version::new(0, 1, 0)).unwrap();
    fs::create_dir(data.join("system/apps/unregistered.lunaapp")).unwrap();
    assert!(manager.list_bounded(64).unwrap().is_empty());
    fs::create_dir(data.join("system/apps/.bundles-v2/unregistered.lunaapp")).unwrap();
    assert!(manager.list_bounded(64).is_err());
}
