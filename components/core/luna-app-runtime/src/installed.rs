//! Verified installed Bundle -> authorized execution plan. No implicit grants.
use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use luna_app_manager::InstalledBundle;
use luna_bundle::{BundleKind, BundleManifest, BundleMetadata, BundleResource};
use luna_common::{ResourceAccess, RuntimeSpec};
use luna_root_mapping::{LogicalPath, MappingRule, MappingTable, PhysicalPath};
use luna_security::{
    AuthorizationRequest, BundleTrustScope, Permission, PolicyAuthority, Principal, Resource,
};
use luna_user_session::UserSession;

use crate::{
    ApplicationPlan, AuthorizedApplicationPlan, ElfClass, ElfDependencyClosure, ElfEndian,
    ElfMachine, ExecutableSpec, FilesystemElfResolver, RuntimeError, TrustedElfSource,
    authorize_application_plan,
    wayland::{WAYLAND_CAPABILITY, WAYLAND_LOGICAL, WaylandEndpoint},
};
use crate::file_handoff::{ApplicationEndpoint, SelectedFile, OPEN_CAPABILITY, SELECTED_CAPABILITY, SELECTED_LOGICAL};
use crate::protocol::APPLICATION_SOCKET;

pub const HOME_CAPABILITY: &str = "user.home";
pub const TERMINAL_CAPABILITY: &str = "terminal.pty";
pub const HOME_LOGICAL: &str = "/home";

/// The manager must retain its registry lock until the child completes exec.
/// The mount namespace then pins the mounted resources independently of paths.
pub fn authorize_installed_bundle(
    installed: &InstalledBundle,
    session: &UserSession,
    storage: &Path,
    args: &[OsString],
    policy: &dyn PolicyAuthority,
) -> Result<AuthorizedApplicationPlan, RuntimeError> {
    authorize_installed_bundle_with_wayland(installed, session, storage, args, policy, None)
}

pub fn authorize_installed_bundle_with_wayland(
    installed: &InstalledBundle,
    session: &UserSession,
    storage: &Path,
    args: &[OsString],
    policy: &dyn PolicyAuthority,
    wayland: Option<&WaylandEndpoint>,
) -> Result<AuthorizedApplicationPlan, RuntimeError> {
    let plan = installed_bundle_plan(
        installed,
        session,
        storage,
        args,
        wayland,
        PlanningPurpose::Launch,
        None,
        None,
    )?;
    authorize_application_plan(plan, policy).map_err(|e| RuntimeError::Security(e.to_string()))
}

/// Discovery checks the same ELF, mappings and policy requests without probing
/// a compositor, creating storage, or exposing an executable authorized plan.
pub fn validate_installed_desktop_application(
    installed: &InstalledBundle,
    session: &UserSession,
    storage: &Path,
    policy: &dyn PolicyAuthority,
) -> Result<(), RuntimeError> {
    let plan = installed_bundle_plan(
        installed,
        session,
        storage,
        &[],
        None,
        PlanningPurpose::Discovery,
        None,
        None,
    )?;
    luna_security::authorize(policy, plan)
        .map(|_| ())
        .map_err(|e| RuntimeError::Security(e.to_string()))
}

#[derive(PartialEq)]
enum PlanningPurpose {
    Launch,
    Discovery,
    FileLaunch,
    FileDiscovery,
}

/// Расширенный путь существующего адаптера: доказательства ресурсов не заменяют policy.
#[allow(clippy::too_many_arguments)]
pub fn authorize_installed_bundle_with_handoff(
    installed: &InstalledBundle, session: &UserSession, storage: &Path, args: &[OsString],
    policy: &dyn PolicyAuthority, wayland: Option<&WaylandEndpoint>,
    endpoint: Option<&ApplicationEndpoint>, selected: Option<&SelectedFile>,
) -> Result<AuthorizedApplicationPlan, RuntimeError> {
    let purpose = if selected.is_some() { PlanningPurpose::FileLaunch } else { PlanningPurpose::Launch };
    let plan = installed_bundle_plan(installed, session, storage, args, wayland, purpose, endpoint, selected)?;
    authorize_application_plan(plan, policy).map_err(|e| RuntimeError::Security(e.to_string()))
}

pub fn validate_installed_file_handler(installed: &InstalledBundle, session: &UserSession,
    storage: &Path, policy: &dyn PolicyAuthority) -> Result<(), RuntimeError> {
    let plan = installed_bundle_plan(installed, session, storage, &[], None,
        PlanningPurpose::FileDiscovery, None, None)?;
    luna_security::authorize(policy, plan).map(|_| ()).map_err(|e| RuntimeError::Security(e.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn installed_bundle_plan(
    installed: &InstalledBundle,
    session: &UserSession,
    storage: &Path,
    args: &[OsString],
    wayland: Option<&WaylandEndpoint>,
    purpose: PlanningPurpose,
    endpoint: Option<&ApplicationEndpoint>,
    selected: Option<&SelectedFile>,
) -> Result<ApplicationPlan, RuntimeError> {
    let info = &installed.inspection;
    let source = &info.manifest;
    if source.platform.arch != "x86_64"
        || source.bundle.kind != "application"
        || !source.dependencies.is_empty()
        || source.capabilities.requested.iter().any(|cap| {
            ![WAYLAND_CAPABILITY, HOME_CAPABILITY, TERMINAL_CAPABILITY, OPEN_CAPABILITY].contains(&cap.as_str())
        })
    {
        return Err(RuntimeError::InvalidBundle(
            "launch requires an x86_64 application with supported native resources".into(),
        ));
    }
    let entry = source
        .entry
        .as_ref()
        .ok_or_else(|| RuntimeError::InvalidExecutable("missing entry".into()))?;
    let executable = entry.logical.as_ref().ok_or_else(|| {
        RuntimeError::InvalidExecutable("entry requires an explicit logical path".into())
    })?;
    let file_mode = matches!(purpose, PlanningPurpose::FileLaunch | PlanningPurpose::FileDiscovery);
    let file_entry = if file_mode { Some(entry.file_open.as_ref().ok_or_else(||
        RuntimeError::InvalidBundle("application has no file-open mode".into()))?) } else { None };
    if file_mode && !args.is_empty() { return Err(RuntimeError::InvalidExecutable("file-open mode rejects caller arguments".into())); }
    let file_args = file_entry.map_or(&[][..], |entry| entry.args.as_slice());
    if let Some(entry) = file_entry { entry.validate().map_err(|e| RuntimeError::InvalidBundle(e.to_string()))?; }
    if entry.args.len() + args.len() + file_args.len() + usize::from(file_mode) > 64
        || entry
            .args
            .iter()
            .any(|arg| arg.len() > 4096 || arg.contains('\0'))
        || args
            .iter()
            .any(|arg| arg.len() > 4096 || arg.as_bytes().contains(&0))
    {
        return Err(RuntimeError::InvalidExecutable(
            "invalid combined application arguments".into(),
        ));
    }
    let application = info.application.id().clone();
    let mut manifest = BundleManifest::new(BundleMetadata::new(
        application.clone(),
        info.application.version(),
        BundleKind::Application,
    ));
    let needs_wayland = source
        .capabilities
        .requested
        .iter()
        .any(|cap| cap == WAYLAND_CAPABILITY);
    let discovery = matches!(purpose, PlanningPurpose::Discovery | PlanningPurpose::FileDiscovery);
    let needs_endpoint = !file_mode && source.capabilities.requested.iter().any(|cap| cap == OPEN_CAPABILITY);
    if (!discovery && needs_wayland != wayland.is_some())
        || wayland.is_some_and(|proof| proof.session() != session.id())
        || (!discovery && needs_endpoint != endpoint.is_some())
        || (purpose == PlanningPurpose::FileLaunch && selected.is_none())
    {
        return Err(RuntimeError::Security(
            "Wayland resource does not match the requested session capability".into(),
        ));
    }
    for capability in &source.capabilities.requested {
        // Режим просмотра урезает права: HOME и запуск других приложений не наследуются.
        if file_mode && [HOME_CAPABILITY, OPEN_CAPABILITY].contains(&capability.as_str()) { continue; }
        manifest.add_capability(capability);
    }
    if file_mode { manifest.add_capability(SELECTED_CAPABILITY); }
    let mut mapping = MappingTable::new();
    mapping
        .bind_runtime(RuntimeSpec::musl().kind())
        .map_err(RuntimeError::Mapping)?;
    // These are musl's defaults, searched only inside explicit Bundle mappings.
    // No host loader, ld.so.cache, environment or system library fallback.
    let mut resolver = FilesystemElfResolver::new();
    for path in ["/lib", "/usr/local/lib", "/usr/lib"] {
        resolver = resolver
            .with_default_search_path(path)
            .map_err(|error| RuntimeError::InvalidExecutable(error.to_string()))?;
    }
    for declaration in &source.mappings {
        let path = Path::new(&declaration.logical);
        if [
            "/data", "/config", "/home", "/open", "/proc", "/sys", "/dev", "/tmp", "/run", "/.luna",
        ]
        .iter()
        .any(|reserved| path.starts_with(reserved))
        {
            return Err(RuntimeError::InvalidBundle(
                "Bundle mapping overlaps a reserved runtime path".into(),
            ));
        }
        let access = declaration
            .access
            .iter()
            .map(|access| match access.as_str() {
                "read" => Ok(ResourceAccess::Read),
                "execute" => Ok(ResourceAccess::Execute),
                _ => Err(RuntimeError::InvalidBundle(
                    "payload mappings must be read-only".into(),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let physical = installed.payload_root.join(&declaration.source);
        // Directory codec/registry already verified every payload object. Map
        // one declared object, never the whole hidden DATA or System Image.
        let rule = if physical.is_dir() {
            MappingRule::subtree(
                LogicalPath::new(path).map_err(RuntimeError::Mapping)?,
                PhysicalPath::new(&physical),
            )
        } else {
            MappingRule::file(
                LogicalPath::new(path).map_err(RuntimeError::Mapping)?,
                PhysicalPath::new(&physical),
            )
        }
        .with_access(access.iter().copied());
        mapping.insert(rule).map_err(RuntimeError::Mapping)?;
        manifest.add_resource(
            BundleResource::new(&declaration.logical, &declaration.source).with_access(access),
        );
        resolver = resolver.with_source(
            TrustedElfSource::new(path, physical)
                .map_err(|e| RuntimeError::InvalidExecutable(e.to_string()))?,
        );
    }
    if !source
        .mappings
        .iter()
        .any(|m| &m.logical == executable && m.source == entry.exec)
    {
        return Err(RuntimeError::InvalidExecutable(
            "entry source does not match its logical mapping".into(),
        ));
    }
    let closure = ElfDependencyClosure::build(executable, &resolver)
        .map_err(|e| RuntimeError::InvalidExecutable(e.to_string()))?;
    let dynamic = closure
        .nodes()
        .any(|node| node.metadata().has_dynamic_loader());
    if closure.nodes().any(|node| {
        let elf = node.metadata();
        elf.class() != ElfClass::Class64
            || elf.endian() != ElfEndian::Little
            || elf.machine() != ElfMachine::X86_64
            || elf
                .interpreter()
                .is_some_and(|path| path != Path::new("/lib/ld-musl-x86_64.so.1"))
            || elf.has_search_path_tags()
    }) {
        return Err(RuntimeError::InvalidExecutable(
            "installed launch requires x86_64 musl ELF without RPATH/RUNPATH".into(),
        ));
    }
    if dynamic {
        let entry_node = closure
            .nodes()
            .find(|node| node.path() == Path::new(executable))
            .expect("closure contains its root");
        if entry_node.metadata().interpreter() != Some(Path::new("/lib/ld-musl-x86_64.so.1"))
            || source.mappings.iter().any(|declaration| {
                Path::new("/etc/ld-musl-x86_64.path").starts_with(Path::new(&declaration.logical))
            })
        {
            return Err(RuntimeError::InvalidExecutable(
                "dynamic musl requires its canonical interpreter and default search path".into(),
            ));
        }
        // The verified Bundle/ContentIdentity is the provenance of every node.
        // A directory grant alone cannot authorize execution of a dependency.
        for node in closure.nodes() {
            let logical = LogicalPath::new(node.path()).map_err(RuntimeError::Mapping)?;
            let rule = mapping
                .resolve_rule(&logical)
                .map_err(RuntimeError::Mapping)?;
            if ![ResourceAccess::Read, ResourceAccess::Execute]
                .iter()
                .all(|access| rule.access().contains(access))
            {
                return Err(RuntimeError::InvalidExecutable(format!(
                    "ELF dependency requires read/execute mapping: {}",
                    node.path().display()
                )));
            }
        }
    }
    if let Some(proof) = wayland {
        mapping
            .insert(
                MappingRule::file(
                    LogicalPath::new(WAYLAND_LOGICAL).map_err(RuntimeError::Mapping)?,
                    PhysicalPath::new(proof.source()),
                )
                .with_access([ResourceAccess::Read]),
            )
            .map_err(RuntimeError::Mapping)?;
        manifest.add_resource(
            BundleResource::new(WAYLAND_LOGICAL, "runtime/wayland")
                .with_access([ResourceAccess::Read]),
        );
    }
    for (logical, source) in endpoint.map(|_| (APPLICATION_SOCKET, Path::new(APPLICATION_SOCKET))).into_iter()
        .chain(selected.map(|file| (SELECTED_LOGICAL, file.source()))) {
        mapping.insert(MappingRule::file(LogicalPath::new(logical).map_err(RuntimeError::Mapping)?, PhysicalPath::new(source))
            .with_access([ResourceAccess::Read])).map_err(RuntimeError::Mapping)?;
        manifest.add_resource(BundleResource::new(logical, "runtime/handoff").with_access([ResourceAccess::Read]));
    }
    for (logical, physical) in private_paths(storage, application.as_str()) {
        mapping
            .insert(
                MappingRule::subtree(
                    LogicalPath::new(logical).map_err(RuntimeError::Mapping)?,
                    PhysicalPath::new(physical),
                )
                .with_access([ResourceAccess::Read, ResourceAccess::Write]),
            )
            .map_err(RuntimeError::Mapping)?;
        manifest.add_resource(
            BundleResource::new(logical, logical.trim_start_matches('/'))
                .with_access([ResourceAccess::Read, ResourceAccess::Write]),
        );
    }
    if !file_mode && source
        .capabilities
        .requested
        .iter()
        .any(|cap| cap == HOME_CAPABILITY)
    {
        mapping
            .insert(
                MappingRule::subtree(
                    LogicalPath::new(HOME_LOGICAL).map_err(RuntimeError::Mapping)?,
                    PhysicalPath::new(storage.join("home")),
                )
                .with_access([
                    ResourceAccess::Read,
                    ResourceAccess::Write,
                    ResourceAccess::Execute,
                ]),
            )
            .map_err(RuntimeError::Mapping)?;
        manifest.add_resource(
            BundleResource::new(HOME_LOGICAL, "runtime/user-home").with_access([
                ResourceAccess::Read,
                ResourceAccess::Write,
                ResourceAccess::Execute,
            ]),
        );
    }
    let principal = Principal::Application(application.clone());
    let mut requests = vec![
        AuthorizationRequest {
            principal: principal.clone(),
            resource: Resource::Application(application.clone()),
            permission: Permission::Execute,
        },
        AuthorizationRequest {
            principal: principal.clone(),
            resource: Resource::BundleTrust {
                application: application.clone(),
                content_identity: info.content_identity,
                scope: BundleTrustScope::Launch,
            },
            permission: Permission::Use,
        },
        AuthorizationRequest {
            principal: principal.clone(),
            resource: Resource::ApplicationData {
                user: session.user().clone(),
                application,
            },
            permission: Permission::Write,
        },
    ];
    if needs_wayland && discovery {
        requests.push(AuthorizationRequest {
            principal: principal.clone(),
            resource: Resource::FilesystemPath(WAYLAND_LOGICAL.into()),
            permission: Permission::Read,
        });
    }
    for logical in [needs_endpoint.then_some(APPLICATION_SOCKET), (file_mode && discovery).then_some(SELECTED_LOGICAL)].into_iter().flatten() {
        requests.push(AuthorizationRequest { principal: principal.clone(), resource: Resource::FilesystemPath(logical.into()), permission: Permission::Read });
    }
    if dynamic {
        for node in closure.nodes() {
            for permission in [Permission::Read, Permission::Execute] {
                requests.push(AuthorizationRequest {
                    principal: principal.clone(),
                    resource: Resource::FilesystemPath(node.path().to_string_lossy().into_owned()),
                    permission,
                });
            }
        }
    }
    ApplicationPlan::new(
        manifest,
        mapping,
        session,
        RuntimeSpec::musl(),
        ExecutableSpec::new(executable).with_args(
            entry
                .args
                .iter()
                .map(OsString::from)
                .chain(file_args.iter().map(OsString::from))
                .chain(file_mode.then(|| OsString::from(SELECTED_LOGICAL)))
                .chain(args.iter().cloned()),
        ),
        requests,
    )
    .map_err(|e| RuntimeError::InvalidBundle(e.to_string()))
}

pub fn private_paths(storage: &Path, application: &str) -> [(&'static str, PathBuf); 2] {
    [
        ("/data", storage.join("data").join(application)),
        ("/config", storage.join("config").join(application)),
    ]
}
