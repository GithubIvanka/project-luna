//! Thin command-line client for the Luna management backends.

pub struct Cli;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use luna_app_manager::{
    ApplicationManager, ApplicationRef, BundleInspection, BundleRepresentation,
    FilesystemApplicationManager, content_identity_hex, inspect_bundle, parse_version,
};
use luna_bundle::lbp::{LbpManifest, build_from_directory};
use luna_common::{BundleId, Version};
use luna_security::{BundleTrustScope, Permission, Principal, Resource, StaticPolicyAuthority};

pub const HELP: &str = "Project Luna application tools

luna bundle pack MANIFEST.toml PAYLOAD_DIR OUTPUT.lbp
luna bundle inspect FILE.lbp | DIRECTORY.lunaapp
luna app run ID VERSION [--expect-content BLAKE3] [-- ARGS...]
luna app list
luna app --offline-data-root DATA_DIR list
luna app --offline-data-root DATA_DIR install FILE.lbp --trust-content BLAKE3
luna app --offline-data-root DATA_DIR update ID OLD_VERSION FILE.lbp --trust-content BLAKE3
luna app --offline-data-root DATA_DIR verify ID VERSION
luna app --offline-data-root DATA_DIR remove ID VERSION
luna app --offline-data-root DATA_DIR recover

Offline management requires an operator-owned DATA directory and no running
system-runtime using its state database. It does not launch applications.
Install trusts only the exact content identity you explicitly supply.
Removal preserves all user data and configuration.
";

pub fn run(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let parts = args.iter().map(String::as_str).collect::<Vec<_>>();
    match parts.as_slice() {
        [] | ["--help"] | ["-h"] => Ok(HELP.into()),
        ["bundle", "pack", manifest, source, output] => {
            let manifest = LbpManifest::from_toml(&fs::read_to_string(manifest)?)?;
            let bytes = build_from_directory(&manifest, source)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            // File creation is durable before announcing success.
            let parent = Path::new(output)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            fs::File::open(parent)?.sync_all()?;
            Ok(format!(
                "packed {} {}\n",
                manifest.bundle.id, manifest.bundle.version
            ))
        }
        ["bundle", "inspect", path] => Ok(describe(&inspect_bundle(path)?)),
        ["app", "--offline-data-root", root, command, rest @ ..] => {
            // Explicit offline operator action. Grants are narrowly bound to
            // the requested management operation; Bundle capabilities are not
            // converted into application execution grants here.
            let manager = FilesystemApplicationManager::new(root, Version::new(0, 1, 0))?;
            match (*command, rest) {
                ("list", []) => {
                    let mut output = String::new();
                    for bundle in manager.list()? {
                        output.push_str(&describe(&bundle.inspection));
                    }
                    Ok(output)
                }
                ("verify", [id, version]) => {
                    manager.verify(&application(id, version)?)?;
                    Ok(format!("verified {id} {version}\n"))
                }
                ("install", [path, "--trust-content", identity]) => {
                    let inspection = inspect_bundle(path)?;
                    let policy = installation_policy(&inspection, identity)?;
                    let mut store = manager.open_offline_state()?;
                    let installed = manager.install(path, &policy, &mut store)?;
                    Ok(format!("installed {}", describe(&installed.inspection)))
                }
                ("update", [id, version, path, "--trust-content", identity]) => {
                    let inspection = inspect_bundle(path)?;
                    let policy = installation_policy(&inspection, identity)?;
                    let mut store = manager.open_offline_state()?;
                    let installed =
                        manager.update(&application(id, version)?, path, &policy, &mut store)?;
                    Ok(format!(
                        "installed new version; retained {id} {version}\n{}",
                        describe(&installed.inspection)
                    ))
                }
                ("remove", [id, version]) => {
                    let mut policy = StaticPolicyAuthority::new();
                    policy.grant(
                        Principal::System,
                        Resource::Application(BundleId::from(*id)),
                        Permission::Manage,
                    );
                    let mut store = manager.open_offline_state()?;
                    manager.remove(&application(id, version)?, &policy, &mut store)?;
                    Ok(format!("removed {id} {version}; user data retained\n"))
                }
                ("recover", []) => {
                    let mut policy = StaticPolicyAuthority::new();
                    policy.grant(Principal::System, Resource::System, Permission::Manage);
                    for application in manager.pending_applications()? {
                        policy.grant(
                            Principal::System,
                            Resource::Application(application.id().clone()),
                            Permission::Manage,
                        );
                    }
                    let mut store = manager.open_offline_state()?;
                    Ok(format!(
                        "reconciled {} checkpoints\n",
                        manager.recover(&policy, &mut store)?
                    ))
                }
                _ => Err(format!("invalid app command\n{HELP}").into()),
            }
        }
        _ => Err(format!("invalid command\n{HELP}").into()),
    }
}

fn application(id: &str, version: &str) -> Result<ApplicationRef, Box<dyn std::error::Error>> {
    Ok(ApplicationRef::new(
        BundleId::from(id),
        parse_version(version)?,
    ))
}

fn installation_policy(
    inspection: &BundleInspection,
    expected: &str,
) -> Result<StaticPolicyAuthority, Box<dyn std::error::Error>> {
    if expected != content_identity_hex(&inspection.content_identity) {
        return Err("explicit trust identity does not match the inspected Bundle".into());
    }
    let mut policy = StaticPolicyAuthority::new();
    policy.grant(
        Principal::System,
        Resource::Application(inspection.application.id().clone()),
        Permission::Manage,
    );
    policy.grant(
        Principal::System,
        Resource::BundleTrust {
            application: inspection.application.id().clone(),
            content_identity: inspection.content_identity,
            scope: BundleTrustScope::Install,
        },
        Permission::Use,
    );
    Ok(policy)
}

fn describe(inspection: &BundleInspection) -> String {
    format!(
        "{} {}\nname={} representation={}\ncontent_identity={}\ntype={} arch={} signature={}\ncapabilities_requested={} dependencies={}\n",
        inspection.application.id().as_str(),
        inspection.application.version(),
        inspection.manifest.bundle.name,
        match inspection.representation {
            BundleRepresentation::Transport => "lbp1",
            BundleRepresentation::Directory => "directory-v1",
        },
        content_identity_hex(&inspection.content_identity),
        inspection.manifest.bundle.kind,
        inspection.manifest.platform.arch,
        inspection.has_signature,
        inspection.manifest.capabilities.requested.join(","),
        inspection.manifest.dependencies.len()
    )
}

/// Live execution uses the system-owned active session and returns child status.
pub fn run_live(args: &[String]) -> Result<(String, i32), Box<dyn std::error::Error>> {
    if args.iter().map(String::as_str).eq(["app", "list"]) {
        return Ok((
            luna_app_runtime::protocol::catalog_applications()?.to_toml()?,
            0,
        ));
    }
    let request = parse_run_request(args)?;
    let response = luna_app_runtime::protocol::run_application(&request)?;
    if let Some(error) = response.error {
        return Err(error.into());
    }
    let code = response
        .exit_code
        .or(response.signal.map(|signal| 128 + signal))
        .ok_or("missing application exit status")?;
    Ok((
        format!(
            "application instance {} exited with status {code}\n",
            response.instance.ok_or("missing application instance")?
        ),
        code,
    ))
}

fn parse_run_request(
    args: &[String],
) -> Result<luna_app_runtime::protocol::RunRequest, Box<dyn std::error::Error>> {
    if args.len() < 4 || args[0] != "app" || args[1] != "run" {
        return Err("usage: luna app run ID VERSION [-- ARGS...]".into());
    }
    let (pin, offset) = if args.get(4).is_some_and(|value| value == "--expect-content") {
        (
            Some(args.get(5).ok_or("missing content identity")?.clone()),
            6,
        )
    } else {
        (None, 4)
    };
    let extra = if args.len() == offset {
        &args[offset..]
    } else if args.get(offset).is_some_and(|value| value == "--") {
        &args[offset + 1..]
    } else {
        return Err("application arguments must follow --".into());
    };
    let request = luna_app_runtime::protocol::RunRequest {
        format: 1,
        application: args[2].clone(),
        version: args[3].clone(),
        args: extra.to_vec(),
        content_identity: pin,
    };
    request.validate()?;
    Ok(request)
}

#[cfg(test)]
mod desktop_tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).into()).collect()
    }
    #[test]
    fn old_run_and_pinned_desktop_command_parse_without_shell() {
        assert!(
            parse_run_request(&args(&["app", "run", "org.test", "0.1.0"]))
                .unwrap()
                .content_identity
                .is_none()
        );
        let pin = "a".repeat(64);
        let request = parse_run_request(&args(&[
            "app",
            "run",
            "org.test",
            "0.1.0",
            "--expect-content",
            &pin,
            "--",
            "--flag",
            "argument with spaces",
        ]))
        .unwrap();
        assert_eq!(request.content_identity, Some(pin));
        assert_eq!(request.args, ["--flag", "argument with spaces"]);
    }
    #[test]
    fn invalid_pins_and_missing_delimiters_fail_before_connect() {
        for command in [
            vec!["app", "run", "org.test", "0.1.0", "--expect-content"],
            vec!["app", "run", "org.test", "0.1.0", "--expect-content", "bad"],
            vec!["app", "run", "org.test", "0.1.0", "--flag"],
        ] {
            assert!(parse_run_request(&args(&command)).is_err());
        }
    }
}
