use luna_app_manager::{ApplicationRef, BundleInspection, BundleRepresentation, InstalledBundle};
use luna_app_runtime::installed::authorize_installed_bundle;
use luna_bundle::lbp::LbpManifest;
use luna_common::{BundleId, RuntimeKind, UserId, Version};
use luna_security::{BundleTrustScope, Permission, Principal, Resource, StaticPolicyAuthority};
use luna_user_session::{SessionId, SessionState, UserSession};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    installed: InstalledBundle,
    session: UserSession,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "luna-installed-plan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("bin")).unwrap();
        // Planning-only ELF metadata fixture. Real execution is tested in QEMU.
        let mut bytes = vec![0; 64];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[18] = 62;
        fs::write(root.join("bin/probe"), bytes).unwrap();
        fs::set_permissions(root.join("bin/probe"), fs::Permissions::from_mode(0o555)).unwrap();
        let manifest = LbpManifest::from_toml(
            r#"format=1
[bundle]
id="org.luna.probe"
name="Probe"
version="0.1.0"
type="application"
[platform]
arch="x86_64"
[entry]
exec="bin/probe"
logical="/bin/probe"
[[mapping]]
logical="/bin/probe"
source="bin/probe"
access=["read","execute"]
"#,
        )
        .unwrap();
        let installed = InstalledBundle {
            inspection: BundleInspection {
                application: ApplicationRef::new(
                    BundleId::from("org.luna.probe"),
                    Version::new(0, 1, 0),
                ),
                content_identity: [7; 32],
                manifest,
                has_signature: false,
                representation: BundleRepresentation::Directory,
            },
            payload_root: root.clone(),
            directory_root: root.clone(),
        };
        let mut session = UserSession::new(SessionId::new(7), UserId::from("alice"));
        session.transition(SessionState::Authenticating).unwrap();
        session.login_succeeded().unwrap();
        Self {
            root,
            installed,
            session,
        }
    }
    fn policy(&self) -> StaticPolicyAuthority {
        let mut p = StaticPolicyAuthority::new();
        let app = BundleId::from("org.luna.probe");
        let principal = Principal::Application(app.clone());
        p.grant(
            principal.clone(),
            Resource::Runtime(RuntimeKind::Musl),
            Permission::Use,
        );
        p.grant(
            principal.clone(),
            Resource::Application(app.clone()),
            Permission::Execute,
        );
        p.grant(
            principal.clone(),
            Resource::BundleTrust {
                application: app.clone(),
                content_identity: [7; 32],
                scope: BundleTrustScope::Launch,
            },
            Permission::Use,
        );
        p.grant(
            principal.clone(),
            Resource::ApplicationData {
                user: UserId::from("alice"),
                application: app,
            },
            Permission::Write,
        );
        for (path, permissions) in [
            ("/bin/probe", vec![Permission::Read, Permission::Execute]),
            ("/data", vec![Permission::Read, Permission::Write]),
            ("/config", vec![Permission::Read, Permission::Write]),
        ] {
            for permission in permissions {
                p.grant(
                    principal.clone(),
                    Resource::FilesystemPath(path.into()),
                    permission,
                );
            }
        }
        p
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn installed_plan_binds_session_args_runtime_and_only_own_storage() {
    let f = Fixture::new();
    let plan = authorize_installed_bundle(
        &f.installed,
        &f.session,
        &f.root.join("user"),
        &["argument".into()],
        &f.policy(),
    )
    .unwrap();
    assert_eq!(plan.value().runtime().kind(), RuntimeKind::Musl);
    assert_eq!(plan.value().session(), f.session.id());
    assert_eq!(
        plan.value().executable().args(),
        &[std::ffi::OsString::from("argument")]
    );
    assert_eq!(plan.value().mapping().iter().count(), 3);
    assert!(
        !f.root.join("user").exists(),
        "planning never creates storage"
    );
}

#[test]
fn manifest_defaults_precede_caller_args_and_combined_limit_is_checked() {
    let mut f = Fixture::new();
    f.installed.inspection.manifest.entry.as_mut().unwrap().args = vec!["--default".into()];
    let plan = authorize_installed_bundle(
        &f.installed,
        &f.session,
        &f.root.join("user"),
        &["caller".into()],
        &f.policy(),
    )
    .unwrap();
    assert_eq!(
        plan.value().executable().args(),
        &[
            std::ffi::OsString::from("--default"),
            std::ffi::OsString::from("caller")
        ]
    );
    assert!(
        authorize_installed_bundle(
            &f.installed,
            &f.session,
            &f.root.join("user"),
            &vec!["x".into(); 64],
            &f.policy()
        )
        .is_err()
    );
    f.installed.inspection.manifest.entry.as_mut().unwrap().args = vec!["a\0b".into()];
    assert!(
        authorize_installed_bundle(
            &f.installed,
            &f.session,
            &f.root.join("user"),
            &[],
            &f.policy()
        )
        .is_err()
    );
}

#[test]
fn discovery_checks_same_grants_without_endpoint_probe_or_storage() {
    use luna_app_runtime::installed::validate_installed_desktop_application;
    use luna_app_runtime::wayland::{WAYLAND_CAPABILITY, WAYLAND_LOGICAL};
    let mut f = Fixture::new();
    f.installed
        .inspection
        .manifest
        .capabilities
        .requested
        .push(WAYLAND_CAPABILITY.into());
    let storage = f.root.join("user");
    assert!(
        validate_installed_desktop_application(&f.installed, &f.session, &storage, &f.policy())
            .is_err()
    );
    let mut policy = f.policy();
    let principal = Principal::Application(f.installed.inspection.application.id().clone());
    policy.grant(
        principal.clone(),
        Resource::Capability(WAYLAND_CAPABILITY.into()),
        Permission::Use,
    );
    assert!(
        validate_installed_desktop_application(&f.installed, &f.session, &storage, &policy)
            .is_err()
    );
    policy.grant(
        principal,
        Resource::FilesystemPath(WAYLAND_LOGICAL.into()),
        Permission::Read,
    );
    validate_installed_desktop_application(&f.installed, &f.session, &storage, &policy).unwrap();
    assert!(!storage.exists());
    assert!(
        authorize_installed_bundle(&f.installed, &f.session, &storage, &[], &policy).is_err(),
        "real launch still requires a sealed compositor proof"
    );
}
#[test]
fn exact_launch_trust_is_required_even_after_install() {
    let f = Fixture::new();
    let mut p = f.policy();
    let resource = Resource::BundleTrust {
        application: BundleId::from("org.luna.probe"),
        content_identity: [7; 32],
        scope: BundleTrustScope::Launch,
    };
    p.revoke(
        &Principal::Application(BundleId::from("org.luna.probe")),
        &resource,
        Permission::Use,
    );
    p.grant(
        Principal::Application(BundleId::from("org.luna.probe")),
        Resource::BundleTrust {
            application: BundleId::from("org.luna.probe"),
            content_identity: [7; 32],
            scope: BundleTrustScope::Install,
        },
        Permission::Use,
    );
    assert!(authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &p).is_err());
}
#[test]
fn changed_content_identity_loses_launch_grant() {
    let mut f = Fixture::new();
    let p = f.policy();
    f.installed.inspection.content_identity = [8; 32];
    assert!(authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &p).is_err());
}
#[test]
fn mapping_permission_is_required_separately() {
    let f = Fixture::new();
    let mut p = f.policy();
    p.revoke(
        &Principal::Application(BundleId::from("org.luna.probe")),
        &Resource::FilesystemPath("/config".into()),
        Permission::Write,
    );
    assert!(authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &p).is_err());
}
#[test]
fn reserved_runtime_and_writable_payload_mappings_fail_closed() {
    for (path, access) in [
        ("/data/foreign", vec!["read".into()]),
        (
            "/bin/probe",
            vec!["read".into(), "write".into(), "execute".into()],
        ),
    ] {
        let mut f = Fixture::new();
        f.installed.inspection.manifest.mappings[0].logical = path.into();
        f.installed.inspection.manifest.mappings[0].access = access;
        assert!(
            authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &f.policy())
                .is_err()
        );
    }
}
#[test]
fn foreign_architecture_and_inactive_session_fail_closed() {
    let mut f = Fixture::new();
    let mut bytes = fs::read(f.root.join("bin/probe")).unwrap();
    bytes[18] = 183;
    fs::set_permissions(f.root.join("bin/probe"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(f.root.join("bin/probe"), bytes).unwrap();
    assert!(
        authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &f.policy()).is_err()
    );
    let mut bytes = fs::read(f.root.join("bin/probe")).unwrap();
    bytes[18] = 62;
    fs::write(f.root.join("bin/probe"), bytes).unwrap();
    f.session.transition(SessionState::Ending).unwrap();
    assert!(
        authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &f.policy()).is_err()
    );
}

// Planning metadata only: real dynamic loader and Wayland execution are QEMU gates.
fn dynamic_elf(needed: &[&str], interpreter: Option<&str>, search_tag: Option<u64>) -> Vec<u8> {
    let mut bytes = vec![0; 0x700];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[18] = 62;
    let set = |bytes: &mut [u8], at: usize, value: u64| {
        bytes[at..at + 8].copy_from_slice(&value.to_le_bytes())
    };
    set(&mut bytes, 32, 64);
    bytes[54] = 56;
    bytes[56] = 3;
    for (at, kind, offset, size) in [
        (64, 1u32, 0, 0x700),
        (120, 3, 0x200, interpreter.map_or(0, |v| v.len() + 1) as u64),
        (176, 2, 0x300, 0x100),
    ] {
        bytes[at..at + 4].copy_from_slice(&kind.to_le_bytes());
        set(&mut bytes, at + 8, offset);
        set(&mut bytes, at + 16, 0x400000 + offset);
        set(&mut bytes, at + 32, size);
        set(&mut bytes, at + 40, size);
    }
    if let Some(value) = interpreter {
        bytes[0x200..0x200 + value.len()].copy_from_slice(value.as_bytes());
    } else {
        bytes[120..124].fill(0);
    }
    let mut strings = vec![0];
    let mut at = 0x320;
    for value in needed {
        set(&mut bytes, at, 1);
        set(&mut bytes, at + 8, strings.len() as u64);
        at += 16;
        strings.extend_from_slice(value.as_bytes());
        strings.push(0);
    }
    if let Some(tag) = search_tag {
        set(&mut bytes, at, tag);
        set(&mut bytes, at + 8, 0);
    }
    set(&mut bytes, 0x300, 5);
    set(&mut bytes, 0x308, 0x400500);
    set(&mut bytes, 0x310, 10);
    set(&mut bytes, 0x318, strings.len() as u64);
    bytes[0x500..0x500 + strings.len()].copy_from_slice(&strings);
    bytes
}
impl Fixture {
    fn dynamic(&mut self) {
        fs::set_permissions(
            self.root.join("bin/probe"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        fs::create_dir(self.root.join("lib")).unwrap();
        fs::write(
            self.root.join("bin/probe"),
            dynamic_elf(&["libprobe.so"], Some("/lib/ld-musl-x86_64.so.1"), None),
        )
        .unwrap();
        for name in ["ld-musl-x86_64.so.1", "libprobe.so"] {
            fs::write(
                self.root.join("lib").join(name),
                dynamic_elf(&[], None, None),
            )
            .unwrap();
            self.installed.inspection.manifest.mappings.push(
                luna_bundle::lbp::MappingDeclaration {
                    logical: format!("/lib/{name}"),
                    source: format!("lib/{name}"),
                    access: vec!["read".into(), "execute".into()],
                },
            );
        }
    }
    fn dynamic_policy(&self) -> StaticPolicyAuthority {
        let mut policy = self.policy();
        for name in ["ld-musl-x86_64.so.1", "libprobe.so"] {
            for access in [Permission::Read, Permission::Execute] {
                policy.grant(
                    Principal::Application(BundleId::from("org.luna.probe")),
                    Resource::FilesystemPath(format!("/lib/{name}")),
                    access,
                );
            }
        }
        policy
    }
}
#[test]
fn dynamic_musl_requires_every_dependency_mapping_and_exact_grant() {
    let mut f = Fixture::new();
    f.dynamic();
    let mut policy = f.dynamic_policy();
    let plan = authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &policy).unwrap();
    assert_eq!(plan.value().mapping().iter().count(), 5);
    policy.revoke(
        &Principal::Application(BundleId::from("org.luna.probe")),
        &Resource::FilesystemPath("/lib/libprobe.so".into()),
        Permission::Execute,
    );
    assert!(authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &policy).is_err());
    f.installed.inspection.manifest.mappings.pop();
    assert!(
        authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &f.dynamic_policy())
            .is_err()
    );
}
#[test]
fn dynamic_runtime_custom_search_path_and_empty_tags_fail_closed() {
    for tag in [15, 29] {
        let mut f = Fixture::new();
        f.dynamic();
        fs::write(
            f.root.join("bin/probe"),
            dynamic_elf(
                &["libprobe.so"],
                Some("/lib/ld-musl-x86_64.so.1"),
                Some(tag),
            ),
        )
        .unwrap();
        assert!(
            authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &f.dynamic_policy())
                .is_err()
        );
    }
    for path in ["/etc", "/etc/ld-musl-x86_64.path"] {
        let mut f = Fixture::new();
        f.dynamic();
        fs::create_dir(f.root.join("etc")).unwrap();
        fs::write(f.root.join("etc/ld-musl-x86_64.path"), "/host/lib").unwrap();
        f.installed
            .inspection
            .manifest
            .mappings
            .push(luna_bundle::lbp::MappingDeclaration {
                logical: path.into(),
                source: path.trim_start_matches('/').into(),
                access: vec!["read".into()],
            });
        assert!(
            authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &f.dynamic_policy())
                .is_err()
        );
    }
}
#[test]
fn dynamic_dependencies_reject_foreign_elf_and_missing_execute_access() {
    let mut f = Fixture::new();
    f.dynamic();
    f.installed
        .inspection
        .manifest
        .mappings
        .last_mut()
        .unwrap()
        .access = vec!["read".into()];
    assert!(
        authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &f.dynamic_policy())
            .is_err()
    );
    f.installed
        .inspection
        .manifest
        .mappings
        .last_mut()
        .unwrap()
        .access
        .push("execute".into());
    let mut bytes = dynamic_elf(&[], None, None);
    bytes[18] = 183;
    fs::write(f.root.join("lib/libprobe.so"), bytes).unwrap();
    assert!(
        authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &f.dynamic_policy())
            .is_err()
    );
}

#[test]
fn wayland_requires_sealed_capability_path_session_uid_and_same_resource() {
    use luna_app_runtime::{
        installed::authorize_installed_bundle_with_wayland,
        wayland::{WAYLAND_CAPABILITY, WAYLAND_LOGICAL, WaylandEndpoint},
    };
    use luna_user_session::UserCredentials;
    use std::os::unix::net::UnixListener;
    let mut f = Fixture::new();
    f.installed
        .inspection
        .manifest
        .capabilities
        .requested
        .push(WAYLAND_CAPABILITY.into());
    let uid = unsafe { libc::geteuid() };
    let source = f.root.join("wayland-0");
    let _listener = UnixListener::bind(&source).unwrap();
    let proof = WaylandEndpoint::inspect(source, f.session.id(), uid).unwrap();
    let principal = Principal::Application(BundleId::from("org.luna.probe"));
    let mut policy = f.policy();
    assert!(
        authorize_installed_bundle_with_wayland(
            &f.installed,
            &f.session,
            &f.root,
            &[],
            &policy,
            Some(&proof)
        )
        .is_err()
    );
    policy.grant(
        principal.clone(),
        Resource::Capability(WAYLAND_CAPABILITY.into()),
        Permission::Use,
    );
    assert!(
        authorize_installed_bundle_with_wayland(
            &f.installed,
            &f.session,
            &f.root,
            &[],
            &policy,
            Some(&proof)
        )
        .is_err()
    );
    policy.grant(
        principal,
        Resource::FilesystemPath(WAYLAND_LOGICAL.into()),
        Permission::Read,
    );
    let plan = authorize_installed_bundle_with_wayland(
        &f.installed,
        &f.session,
        &f.root,
        &[],
        &policy,
        Some(&proof),
    )
    .unwrap();
    assert!(
        proof
            .validate_plan(&plan, &UserCredentials::new(uid, uid, "alice"))
            .is_ok()
    );
    assert!(
        proof
            .validate_plan(&plan, &UserCredentials::new(uid + 1, uid, "alice"))
            .is_err()
    );
    let foreign = WaylandEndpoint::inspect(proof.source().into(), SessionId::new(8), uid).unwrap();
    assert!(
        foreign
            .validate_plan(&plan, &UserCredentials::new(uid, uid, "alice"))
            .is_err()
    );
    assert!(
        authorize_installed_bundle_with_wayland(
            &f.installed,
            &f.session,
            &f.root,
            &[],
            &policy,
            Some(&foreign)
        )
        .is_err()
    );
    let other_path = f.root.join("wayland-1");
    let _other_listener = UnixListener::bind(&other_path).unwrap();
    let other = WaylandEndpoint::inspect(other_path, f.session.id(), uid).unwrap();
    assert!(
        other
            .validate_plan(&plan, &UserCredentials::new(uid, uid, "alice"))
            .is_err()
    );
    assert!(authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &policy).is_err());
}

#[test]
fn user_home_requires_capability_and_path_grants_and_binds_only_session_storage() {
    use luna_app_runtime::installed::{HOME_CAPABILITY, HOME_LOGICAL};
    let mut f = Fixture::new();
    f.installed
        .inspection
        .manifest
        .capabilities
        .requested
        .push(HOME_CAPABILITY.into());
    let storage = f.root.join("alice");
    let mut policy = f.policy();
    assert!(authorize_installed_bundle(&f.installed, &f.session, &storage, &[], &policy).is_err());
    let principal = Principal::Application(f.installed.inspection.application.id().clone());
    policy.grant(
        principal.clone(),
        Resource::Capability(HOME_CAPABILITY.into()),
        Permission::Use,
    );
    assert!(authorize_installed_bundle(&f.installed, &f.session, &storage, &[], &policy).is_err());
    for permission in [Permission::Read, Permission::Write, Permission::Execute] {
        policy.grant(
            principal.clone(),
            Resource::FilesystemPath(HOME_LOGICAL.into()),
            permission,
        );
    }
    let plan =
        authorize_installed_bundle(&f.installed, &f.session, &storage, &[], &policy).unwrap();
    let rule = plan
        .value()
        .mapping()
        .iter()
        .find(|rule| rule.logical().as_str() == HOME_LOGICAL)
        .unwrap();
    assert_eq!(rule.physical().unwrap().as_path(), storage.join("home"));
}

#[test]
fn terminal_pty_request_is_not_a_grant() {
    use luna_app_runtime::installed::TERMINAL_CAPABILITY;
    let mut f = Fixture::new();
    f.installed
        .inspection
        .manifest
        .capabilities
        .requested
        .push(TERMINAL_CAPABILITY.into());
    let mut policy = f.policy();
    assert!(authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &policy).is_err());
    policy.grant(
        Principal::Application(f.installed.inspection.application.id().clone()),
        Resource::Capability(TERMINAL_CAPABILITY.into()),
        Permission::Use,
    );
    assert!(authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &policy).is_ok());
}

#[test]
fn bundle_cannot_supply_home_or_private_pty_paths() {
    for logical in ["/home", "/home/another-user", "/dev/ptmx", "/dev/pts"] {
        let mut f = Fixture::new();
        f.installed.inspection.manifest.mappings[0].logical = logical.into();
        assert!(
            authorize_installed_bundle(&f.installed, &f.session, &f.root, &[], &f.policy())
                .is_err()
        );
    }
}

// Отдельный неизменяемый режим просмотра урезает HOME, а не выдаёт новые права.
fn file_mode_fixture() -> Fixture {
    let mut fixture = Fixture::new();
    fixture.installed.inspection.manifest.entry.as_mut().unwrap().file_open = Some(luna_bundle::lbp::FileOpenEntry {
        mime_types: vec!["text/plain".into()], args: vec!["--view".into()],
    });
    fixture.installed.inspection.manifest.capabilities.requested.push("user.home".into());
    fixture
}
fn file_mode_policy(fixture: &Fixture) -> StaticPolicyAuthority {
    let mut policy = fixture.policy();
    let principal = Principal::Application(fixture.installed.inspection.application.id().clone());
    for capability in ["user.home", "user.selected-file"] {
        policy.grant(principal.clone(), Resource::Capability(capability.into()), Permission::Use);
    }
    policy.grant(principal.clone(), Resource::FilesystemPath("/open/selected".into()), Permission::Read);
    for permission in [Permission::Read, Permission::Write, Permission::Execute] {
        policy.grant(principal.clone(), Resource::FilesystemPath("/home".into()), permission);
    }
    policy
}
#[test]
fn file_discovery_requires_its_own_grants_without_creating_storage() {
    use luna_app_runtime::installed::validate_installed_file_handler;
    let fixture = file_mode_fixture();
    let storage = fixture.root.join("missing-user-storage");
    assert!(validate_installed_file_handler(&fixture.installed, &fixture.session, &storage, &fixture.policy()).is_err());
    validate_installed_file_handler(&fixture.installed, &fixture.session, &storage, &file_mode_policy(&fixture)).unwrap();
    assert!(!storage.exists());
}
#[test]
fn file_launch_has_exact_read_resource_without_home_and_preserves_normal_run() {
    use luna_app_runtime::file_handoff::{SelectedFile, SELECTED_LOGICAL};
    use luna_app_runtime::installed::authorize_installed_bundle_with_handoff;
    let fixture = file_mode_fixture();
    std::fs::create_dir(fixture.root.join("home")).unwrap();
    std::fs::write(fixture.root.join("home/chosen"), b"read-only view").unwrap();
    // SAFETY: запросы uid/gid не принимают указателей и не меняют права процесса.
    let (uid, gid) = unsafe { (libc::geteuid(), libc::getegid()) };
    let file = SelectedFile::inspect(&fixture.root.join("home"), std::path::Path::new("/home/chosen"), fixture.session.id(), uid).unwrap();
    let policy = file_mode_policy(&fixture);
    assert!(authorize_installed_bundle_with_handoff(&fixture.installed, &fixture.session, &fixture.root,
        &[std::ffi::OsString::from("--untrusted")], &policy, None, None, Some(&file)).is_err());
    let plan = authorize_installed_bundle_with_handoff(&fixture.installed, &fixture.session, &fixture.root, &[], &policy, None, None, Some(&file)).unwrap();
    assert!(!plan.value().manifest().capabilities().any(|cap| cap == "user.home"));
    assert!(!plan.value().mapping().iter().any(|rule| rule.logical().as_str() == "/home"));
    assert_eq!(plan.value().executable().args(), &[std::ffi::OsString::from("--view"), std::ffi::OsString::from(SELECTED_LOGICAL)]);
    file.validate_plan(&plan, &luna_user_session::UserCredentials::new(uid, gid, "alice")).unwrap();
    assert!(file.validate_plan(&plan, &luna_user_session::UserCredentials::new(uid+1, gid, "alice")).is_err());
    let rule = plan.value().mapping().iter().find(|rule| rule.logical().as_str() == SELECTED_LOGICAL).unwrap();
    assert_eq!(rule.access().len(), 1); assert!(rule.access().contains(&luna_common::ResourceAccess::Read));
    let normal = authorize_installed_bundle(&fixture.installed, &fixture.session, &fixture.root, &[], &policy).unwrap();
    assert!(normal.value().manifest().capabilities().any(|cap| cap == "user.home"));
    assert!(!normal.value().manifest().capabilities().any(|cap| cap == "user.selected-file"));
    assert!(normal.value().executable().args().is_empty());
}
#[test]
fn file_launch_rejects_caller_args_and_reserved_payload_prefix() {
    use luna_app_runtime::installed::validate_installed_file_handler;
    let mut fixture = file_mode_fixture();
    let policy = file_mode_policy(&fixture);
    fixture.installed.inspection.manifest.mappings[0].logical = "/open/hidden".into();
    fixture.installed.inspection.manifest.entry.as_mut().unwrap().logical = Some("/open/hidden".into());
    assert!(validate_installed_file_handler(&fixture.installed, &fixture.session, &fixture.root, &policy).is_err());
}
#[test]
fn file_mode_metadata_is_optional_canonical_and_bounded() {
    let mut fixture = Fixture::new();
    let old = fixture.installed.inspection.manifest.to_toml().unwrap();
    assert!(!old.contains("file_open"));
    fixture.installed.inspection.manifest.entry.as_mut().unwrap().file_open = Some(luna_bundle::lbp::FileOpenEntry {
        mime_types: vec!["text/plain".into()], args: vec!["--view".into()],
    });
    let new = fixture.installed.inspection.manifest.to_toml().unwrap();
    assert!(new.contains("[entry.file_open]")); assert_ne!(old, new);
    assert_eq!(luna_bundle::lbp::LbpManifest::from_toml(&new).unwrap(), fixture.installed.inspection.manifest);
    fixture.installed.inspection.manifest.entry.as_mut().unwrap().file_open.as_mut().unwrap().mime_types = vec!["text/*".into()];
    assert!(fixture.installed.inspection.manifest.validate().is_err());
    fixture.installed.inspection.manifest.entry.as_mut().unwrap().file_open.as_mut().unwrap().mime_types = vec!["text/plain".into()];
    fixture.installed.inspection.manifest.entry.as_mut().unwrap().args = vec!["arg".into(); 64];
    assert!(fixture.installed.inspection.manifest.validate().is_err());
}
