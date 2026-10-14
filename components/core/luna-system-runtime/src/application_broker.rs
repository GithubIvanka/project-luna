//! Запросы доверенного рабочего стола и ограниченное открытие файлов из приложений.
//! Роль определяется runtime, а не полями запроса; пользователь не выбирает сессию.
use std::collections::BTreeMap;
use std::ffi::{CString, OsString};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::{Duration, Instant};

use luna_app_manager::{
    ApplicationRef, FilesystemApplicationManager, content_identity_hex, parse_version,
};
use luna_app_runtime::{
    ApplicationInstanceId, ApplicationLaunchContext, ApplicationPlanLauncher,
    LinuxApplicationRuntime, ProcessExit,
    installed::{
        HOME_CAPABILITY, HOME_LOGICAL, TERMINAL_CAPABILITY,
        authorize_installed_bundle_with_handoff, private_paths,
        validate_installed_desktop_application, validate_installed_file_handler,
    },
    protocol::{
        APPLICATION_SOCKET, ApplicationRequest, CatalogResponse, DesktopApplication,
        MAX_DESKTOP_APPLICATIONS, MAX_REQUEST, RunRequest, RunResponse,
    },
    wayland::{WAYLAND_CAPABILITY, WAYLAND_LOGICAL, WaylandEndpoint},
};
use luna_app_runtime::file_handoff::{ApplicationEndpoint, SelectedFile, FileHandler, FileHandlersResponse, OpenFileRequest, OPEN_CAPABILITY, SELECTED_CAPABILITY, SELECTED_LOGICAL};
use luna_common::{BundleId, RuntimeKind, RuntimeProfile, Version};
use luna_namespace::LinuxMountNamespace;
use luna_security::{
    AuthorizationRequest, AuthorizationSource, BundleTrustScope, Permission, Principal, Resource,
    StaticPolicyAuthority,
};
use luna_system_runtime::SystemRuntimeService;
use luna_system_runtime::process_cgroup::ProcessCgroup;
use luna_user_session::{SessionId, SessionState, UserCredentials, UserSession};
use serde::Deserialize;

const DATA: &str = "/.luna/data";
const MAX_CLIENTS: usize = 16;
const MAX_DENIED_CLIENTS: usize = 4;
const MAX_APPLICATIONS: usize = 16;

fn client_capacity(denied: bool, authorized_count: usize, denied_count: usize) -> bool {
    if denied {
        denied_count < MAX_DENIED_CLIENTS
    } else {
        authorized_count < MAX_CLIENTS
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchPolicy {
    format: u32,
    application: Vec<ApplicationGrant>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationGrant {
    user: String,
    id: String,
    version: String,
    content_identity: String,
    runtime: String,
    execute: bool,
    private_data: bool,
    #[serde(default)]
    path: Vec<PathGrant>,
    #[serde(default)]
    capabilities: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathGrant {
    logical: String,
    access: Vec<String>,
}

impl LaunchPolicy {
    fn authority(
        &self,
        session: &UserSession,
        application: &ApplicationRef,
        identity: &[u8; 32],
    ) -> io::Result<StaticPolicyAuthority> {
        if self.format != 1 {
            return Err(io::Error::other("unsupported launch policy"));
        }
        let matches = self
            .application
            .iter()
            .filter(|grant| {
                grant.user == session.user().as_str()
                    && grant.id == application.id().as_str()
                    && grant.version == application.version().to_string()
                    && grant.content_identity == content_identity_hex(identity)
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(io::Error::other(
                "no unique exact-content launch grant for this user",
            ));
        }
        let grant = matches[0];
        if grant.runtime != "musl" || !grant.execute || !grant.private_data {
            return Err(io::Error::other(
                "launch requires explicit musl, execute and private-data grants",
            ));
        }
        let principal = Principal::Application(application.id().clone());
        let mut policy = StaticPolicyAuthority::new();
        policy.grant(
            principal.clone(),
            Resource::Application(application.id().clone()),
            Permission::Execute,
        );
        policy.grant(
            principal.clone(),
            Resource::Runtime(RuntimeKind::Musl),
            Permission::Use,
        );
        policy.grant(
            principal.clone(),
            Resource::BundleTrust {
                application: application.id().clone(),
                content_identity: *identity,
                scope: BundleTrustScope::Launch,
            },
            Permission::Use,
        );
        policy.grant(
            principal.clone(),
            Resource::ApplicationData {
                user: session.user().clone(),
                application: application.id().clone(),
            },
            Permission::Write,
        );
        for capability in &grant.capabilities {
            if ![WAYLAND_CAPABILITY, HOME_CAPABILITY, TERMINAL_CAPABILITY, OPEN_CAPABILITY, SELECTED_CAPABILITY]
                .contains(&capability.as_str())
            {
                return Err(io::Error::other("unsupported launch-policy capability"));
            }
            policy.grant(
                principal.clone(),
                Resource::Capability(capability.clone()),
                Permission::Use,
            );
        }
        for path in &grant.path {
            for access in &path.access {
                let permission = match access.as_str() {
                    "read" => Permission::Read,
                    "write" => Permission::Write,
                    "execute" => Permission::Execute,
                    _ => return Err(io::Error::other("unknown launch-policy access")),
                };
                policy.grant(
                    principal.clone(),
                    Resource::FilesystemPath(path.logical.clone()),
                    permission,
                );
            }
        }
        Ok(policy)
    }
}

struct EndpointRequests(Vec<AuthorizationRequest>);
impl AuthorizationSource for EndpointRequests {
    fn authorization_requests(&self) -> Vec<AuthorizationRequest> {
        self.0.clone()
    }
}

struct Client {
    stream: UnixStream,
    input: Vec<u8>,
    deadline: Instant,
    instance: Option<ApplicationInstanceId>,
    output: Vec<u8>,
    sent: usize,
    denied: bool,
    peer: libc::ucred,
    role: ClientRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClientRole { Desktop, Application(ApplicationInstanceId), Denied }

struct LaunchedApplication {
    application: ApplicationRef,
    content_identity: [u8; 32],
    can_open: bool,
}
pub struct ApplicationBroker {
    listener: UnixListener,
    session: SessionId,
    credentials: UserCredentials,
    groups: Vec<u32>,
    clients: Vec<Client>,
    apps: LinuxApplicationRuntime,
    session_membership: String,
    desktop: crate::application_catalog::DesktopCatalog,
    launched: BTreeMap<ApplicationInstanceId, LaunchedApplication>,
    endpoint: ApplicationEndpoint,
}

impl ApplicationBroker {
    pub fn new(
        session: &UserSession,
        credentials: UserCredentials,
        cgroup: &ProcessCgroup,
    ) -> io::Result<Self> {
        if session.state() != SessionState::Active
            || credentials.uid == 0
            || session.user().as_str() != credentials.username
        {
            return Err(io::Error::other(
                "application broker requires an authenticated non-root user",
            ));
        }
        let groups = credentials.supplementary_groups()?;
        let directory = Path::new("/run/luna/app-instances");
        fs::create_dir(directory)?;
        if let Err(error) = fs::set_permissions(directory, fs::Permissions::from_mode(0o700)) {
            let _ = fs::remove_dir(directory);
            return Err(error);
        }
        let listener = match UnixListener::bind(APPLICATION_SOCKET) {
            Ok(listener) => listener,
            Err(error) => {
                let _ = fs::remove_dir(directory);
                return Err(error);
            }
        };
        if let Err(error) = pass_credentials(listener.as_raw_fd()) {
            drop(listener); let _ = fs::remove_file(APPLICATION_SOCKET);
            let _ = fs::remove_dir(directory); return Err(error);
        }
        if let Err(error) =
            fs::set_permissions(APPLICATION_SOCKET, fs::Permissions::from_mode(0o666))
                .and_then(|_| listener.set_nonblocking(true))
        {
            drop(listener);
            let _ = fs::remove_file(APPLICATION_SOCKET);
            let _ = fs::remove_dir(directory);
            return Err(error);
        }
        eprintln!(
            "luna-system-runtime: application broker ready for UserSession {}",
            session.id().get()
        );
        let desktop = match crate::application_catalog::DesktopCatalog::new() {
            Ok(desktop) => desktop,
            Err(error) => {
                drop(listener);
                let _ = fs::remove_file(APPLICATION_SOCKET);
                let _ = fs::remove_dir(directory);
                return Err(error);
            }
        };
        // Закрепляем именно созданное этим посредником имя сокета до запуска клиентов.
        let endpoint = ApplicationEndpoint::inspect(session.id(), credentials.uid)?;
        let mut broker = Self {
            listener,
            session: session.id(),
            credentials,
            groups,
            clients: Vec::new(),
            apps: LinuxApplicationRuntime::new(),
            session_membership: cgroup.membership().to_owned(),
            desktop,
            launched: BTreeMap::new(),
            endpoint,
        };
        // Publish before Noctalia starts. An unavailable/invalid registry or
        // policy leaves an empty menu and does not disable the launch broker.
        if let Err(error) = broker.refresh_catalog(session) {
            eprintln!("luna-system-runtime: desktop catalog unavailable: {error}");
        }
        Ok(broker)
    }

    pub fn poll(&mut self, runtime: &mut SystemRuntimeService) -> io::Result<()> {
        let session = runtime
            .session(self.session)
            .map_err(|e| io::Error::other(e.to_string()))?
            .clone();
        if session.state() != SessionState::Active {
            return Err(io::Error::other("application session ended"));
        }
        self.reconcile_applications(runtime)?;
        for _ in 0..MAX_CLIENTS {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(true)?;
                    let peer = peer_credentials(&stream)?;
                    let role = self.peer_role(&peer, runtime);
                    let denied = role == ClientRole::Denied;
                    let denied_count = self.clients.iter().filter(|client| client.denied).count();
                    if !client_capacity(denied, self.clients.len() - denied_count, denied_count) {
                        let mut stream = stream;
                        let _ = stream.write_all(
                            toml::to_string(&RunResponse::failed(
                                "peer is not the active session user, or client limit reached",
                            ))
                            .expect("serializable response")
                            .as_bytes(),
                        );
                        continue;
                    }
                    // Drain a bounded request before sending the rejection;
                    // closing with unread input otherwise resets the socket
                    // and loses the diagnostic. Denied peers never reach parsing
                    // or the application launch/materialization path.
                    self.clients.push(Client {
                        stream,
                        input: Vec::new(),
                        deadline: Instant::now() + Duration::from_secs(5),
                        instance: None,
                        output: Vec::new(),
                        sent: 0,
                        denied,
                        peer,
                        role,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        for index in 0..self.clients.len() {
            if self.clients[index].instance.is_none() && self.clients[index].output.is_empty() {
                let mut buffer = [0; 4096];
                let mut complete = false;
                let mut failure = None;
                for _ in 0..9 {
                    match receive_request(&self.clients[index].stream, &mut buffer, &self.clients[index].peer) {
                        Ok(0) => {
                            complete = true;
                            break;
                        }
                        Ok(size) => {
                            self.clients[index].input.extend_from_slice(&buffer[..size]);
                            if self.clients[index].input.len() > MAX_REQUEST {
                                failure = Some("application request too large".to_string());
                                break;
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(e) => {
                            failure = Some(e.to_string());
                            break;
                        }
                    }
                }
                if Instant::now() > self.clients[index].deadline {
                    failure = Some("application request timeout".into());
                }
                if let Some(error) = failure {
                    self.respond(index, RunResponse::failed(error));
                } else if complete {
                    if self.clients[index].denied
                        || self.peer_role(&self.clients[index].peer, runtime) != self.clients[index].role {
                        self.respond(
                            index,
                            RunResponse::failed("peer is not the active session user"),
                        );
                    } else {
                        let result = std::str::from_utf8(&self.clients[index].input)
                            .map_err(io::Error::other)
                            .and_then(|text| {
                                toml::from_str::<ApplicationRequest>(text).map_err(io::Error::other)
                            });
                        match result {
                            Ok(ApplicationRequest::OpenFile(request)) => {
                                let result = self.authorize_open_client(self.clients[index].role, &session)
                                    .and_then(|_| request.validate())
                                    .and_then(|_| self.launch_file(runtime, &session, &request));
                                match result {
                                    Ok(id) => self.respond(index, RunResponse { format: 1, instance: Some(id.get().to_string()), exit_code: None, signal: None, error: None }),
                                    Err(error) => { self.respond(index, RunResponse::failed(error)); self.apps.discard_failed_launches(); }
                                }
                            }
                            Ok(ApplicationRequest::Catalog(request)) => {
                                if request.format == 1 && request.operation == "file-handlers" {
                                    match self.authorize_open_client(self.clients[index].role, &session)
                                        .and_then(|_| self.file_handlers(&session))
                                        .and_then(|response| response.to_toml()) {
                                        Ok(text) => self.respond_text(index, text),
                                        Err(error) => self.respond(index, RunResponse::failed(error)),
                                    }
                                } else if request.format != 1 || request.operation != "catalog"
                                    || self.clients[index].role != ClientRole::Desktop {
                                    self.respond(
                                        index,
                                        RunResponse::failed("unsupported catalog request"),
                                    );
                                } else {
                                    match self
                                        .refresh_catalog(&session)
                                        .and_then(|response| response.to_toml())
                                    {
                                        Ok(text) => self.respond_text(index, text),
                                        Err(error) => {
                                            self.respond(index, RunResponse::failed(error))
                                        }
                                    }
                                }
                            }
                            Ok(ApplicationRequest::Run(request)) => {
                                let result = if self.clients[index].role == ClientRole::Desktop {
                                    self.launch(runtime, &session, &request)
                                } else { Err(io::Error::other("Run is restricted to the trusted desktop")) };
                                match result {
                                    Ok(id) => {
                                        self.clients[index].instance = Some(id);
                                        self.clients[index].input.clear();
                                    }
                                    Err(error) => {
                                        self.respond(index, RunResponse::failed(error));
                                        self.apps.discard_failed_launches();
                                    }
                                }
                            }
                            Err(e) => {
                                self.respond(index, RunResponse::failed(e));
                                self.apps.discard_failed_launches();
                            }
                        }
                    }
                }
            }
            let client = &mut self.clients[index];
            if !client.output.is_empty() {
                match client.stream.write(&client.output[client.sent..]) {
                    Ok(size) => client.sent += size,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => client.sent = client.output.len(),
                }
            }
        }
        self.clients.retain(|client| {
            client.output.is_empty()
                || (client.sent < client.output.len() && Instant::now() < client.deadline)
        });
        Ok(())
    }

    // Запуски живут независимо от соединений: ACK просмотра не освобождает процесс.
    fn reconcile_applications(&mut self, runtime: &mut SystemRuntimeService) -> io::Result<()> {
        for (id, _) in self.apps.reconcile(runtime).map_err(io::Error::other)? {
            let instance = self.apps.instance(id).map_err(io::Error::other)?;
            let (exit_code, signal) = match instance.exit() {
                Some(ProcessExit::Exited { code }) => (Some(code), None),
                Some(ProcessExit::Signaled { signal }) => (None, Some(signal)),
                _ => (None, None),
            };
            let error = instance.cleanup().filter(|cleanup| !cleanup.is_success())
                .map(|cleanup| cleanup.message().unwrap_or("cleanup failed").to_owned());
            crate::boot_diagnostic(format_args!(
                "luna-system-runtime: application complete application={} instance={} pid={} exit={exit_code:?} signal={signal:?} cleanup_ok={}",
                instance.application(), id.get(), instance.process().map(|pid| pid.get()).unwrap_or(0),
                instance.cleanup().is_some_and(|cleanup| cleanup.is_success()),
            ));
            let waiters = self.clients.iter().enumerate().filter_map(|(index, client)|
                (client.instance == Some(id)).then_some(index)).collect::<Vec<_>>();
            for index in waiters {
                self.clients[index].instance = None;
                self.respond(index, RunResponse { format: 1, instance: Some(id.get().to_string()),
                    exit_code, signal, error: error.clone() });
            }
            self.launched.remove(&id);
            self.apps.forget_instance(id).map_err(io::Error::other)?;
        }
        Ok(())
    }

    fn peer_role(&self, peer: &libc::ucred, runtime: &SystemRuntimeService) -> ClientRole {
        if peer.uid != self.credentials.uid { return ClientRole::Denied; }
        let membership = fs::read_to_string(format!("/proc/{}/cgroup", peer.pid)).unwrap_or_default();
        if luna_system_runtime::process_cgroup::member_of(&membership, &self.session_membership) {
            return ClientRole::Desktop;
        }
        for (id, launch) in &self.launched {
            if launch.can_open && let Ok(instance) = self.apps.instance(*id)
                && instance.session() == self.session && instance.state() == luna_app_runtime::InstanceState::Running
                && let Some(leader) = instance.process()
                && runtime.application_contains_peer(leader, peer.pid).unwrap_or(false) {
                return ClientRole::Application(*id);
            }
        }
        ClientRole::Denied
    }

    fn authorize_open_client(&self, role: ClientRole, session: &UserSession) -> io::Result<()> {
        let ClientRole::Application(id) = role else { return Err(io::Error::other("file-open requires an authorized application instance")); };
        let launch = self.launched.get(&id).ok_or_else(|| io::Error::other("application instance ended"))?;
        let authority = read_policy()?.authority(session, &launch.application, &launch.content_identity)?;
        let principal = Principal::Application(launch.application.id().clone());
        luna_security::authorize(&authority, EndpointRequests(vec![
            AuthorizationRequest { principal: principal.clone(), resource: Resource::Capability(OPEN_CAPABILITY.into()), permission: Permission::Use },
            AuthorizationRequest { principal: principal.clone(), resource: Resource::Capability(HOME_CAPABILITY.into()), permission: Permission::Use },
            AuthorizationRequest { principal: principal.clone(), resource: Resource::FilesystemPath(HOME_LOGICAL.into()), permission: Permission::Read },
            AuthorizationRequest { principal, resource: Resource::FilesystemPath(APPLICATION_SOCKET.into()), permission: Permission::Read },
        ])).map_err(io::Error::other)?;
        Ok(())
    }

    fn file_handlers(&self, session: &UserSession) -> io::Result<FileHandlersResponse> {
        let manager = FilesystemApplicationManager::new(DATA, Version::new(0, 1, 0)).map_err(io::Error::other)?;
        let installed = manager.list_bounded(MAX_DESKTOP_APPLICATIONS).map_err(io::Error::other)?;
        let policy = read_policy()?;
        let user_root = Path::new(DATA).join("users").join(session.user().as_str());
        let mut handlers = Vec::new();
        for bundle in installed {
            let info = &bundle.inspection;
            let Some(entry) = info.manifest.entry.as_ref().and_then(|entry| entry.file_open.as_ref()) else { continue; };
            if !info.manifest.capabilities.requested.iter().any(|cap| cap == WAYLAND_CAPABILITY) { continue; }
            let Ok(authority) = policy.authority(session, &info.application, &info.content_identity) else { continue; };
            if validate_installed_file_handler(&bundle, session, &user_root, &authority).is_err() { continue; }
            handlers.push(FileHandler { application: DesktopApplication {
                application: info.application.id().to_string(), version: info.application.version().to_string(),
                content_identity: content_identity_hex(&info.content_identity), name: info.manifest.bundle.name.clone(),
                terminal: info.manifest.capabilities.requested.iter().any(|cap| cap == TERMINAL_CAPABILITY),
            }, mime_types: entry.mime_types.clone() });
        }
        let response = FileHandlersResponse { format: 1, handlers, error: None };
        response.to_toml()?;
        Ok(response)
    }

    fn launch_file(&mut self, runtime: &mut SystemRuntimeService, session: &UserSession,
        request: &OpenFileRequest) -> io::Result<ApplicationInstanceId> {
        let run = RunRequest { format: 1, application: request.application.clone(), version: request.version.clone(),
            args: vec![], content_identity: Some(request.content_identity.clone()) };
        self.launch_mode(runtime, session, &run, Some(request))
    }

    fn respond(&mut self, index: usize, response: RunResponse) {
        eprintln!("luna-system-runtime: application result {:?}", response);
        self.respond_text(
            index,
            toml::to_string(&response).expect("serializable response"),
        );
    }

    fn respond_text(&mut self, index: usize, text: String) {
        self.clients[index].output = text.into_bytes();
        self.clients[index].deadline = Instant::now() + Duration::from_secs(5);
    }

    fn refresh_catalog(&mut self, session: &UserSession) -> io::Result<CatalogResponse> {
        let result = self.catalog(session).and_then(|response| {
            self.desktop.publish(&response)?;
            Ok(response)
        });
        if result.is_err() {
            self.desktop.clear()?;
        }
        result
    }

    fn catalog(&self, session: &UserSession) -> io::Result<CatalogResponse> {
        let manager = FilesystemApplicationManager::new(DATA, Version::new(0, 1, 0))
            .map_err(io::Error::other)?;
        let installed = manager
            .list_bounded(MAX_DESKTOP_APPLICATIONS)
            .map_err(io::Error::other)?;
        let policy = read_policy()?;
        let user_root = Path::new(DATA).join("users").join(session.user().as_str());
        let mut applications = Vec::new();
        for bundle in installed {
            let info = &bundle.inspection;
            if !info
                .manifest
                .capabilities
                .requested
                .iter()
                .any(|cap| cap == WAYLAND_CAPABILITY)
            {
                continue;
            }
            let Ok(authority) =
                policy.authority(session, &info.application, &info.content_identity)
            else {
                continue;
            };
            if validate_installed_desktop_application(&bundle, session, &user_root, &authority)
                .is_err()
            {
                continue;
            }
            let application = DesktopApplication {
                application: info.application.id().to_string(),
                version: info.application.version().to_string(),
                content_identity: content_identity_hex(&info.content_identity),
                name: info.manifest.bundle.name.clone(),
                terminal: info
                    .manifest
                    .capabilities
                    .requested
                    .iter()
                    .any(|cap| cap == TERMINAL_CAPABILITY),
            };
            application.validate()?;
            applications.push(application);
        }
        let response = CatalogResponse {
            format: 1,
            applications,
            error: None,
        };
        response.to_toml()?;
        Ok(response)
    }

    fn launch(
        &mut self,
        runtime: &mut SystemRuntimeService,
        session: &UserSession,
        request: &RunRequest,
    ) -> io::Result<ApplicationInstanceId> {
        self.launch_mode(runtime, session, request, None)
    }

    fn launch_mode(&mut self, runtime: &mut SystemRuntimeService, session: &UserSession,
        request: &RunRequest, file_request: Option<&OpenFileRequest>) -> io::Result<ApplicationInstanceId> {
        request.validate()?;
        if self.launched.len() >= MAX_APPLICATIONS { return Err(io::Error::other("active application limit reached")); }
        // DATA выбрана и смонтирована luna-init; запрос не задаёт физическое хранилище.
        // DATA is the root already selected and mounted by luna-init; IPC does
        // not supply a disk, physical path, user, credentials, or session ID.
        let manager = FilesystemApplicationManager::new(DATA, Version::new(0, 1, 0))
            .map_err(io::Error::other)?;
        let application = ApplicationRef::new(
            BundleId::from(request.application.as_str()),
            parse_version(&request.version).map_err(io::Error::other)?,
        );
        let installed = manager.installed(&application).map_err(io::Error::other)?;
        if request
            .content_identity
            .as_ref()
            .is_some_and(|pin| pin != &content_identity_hex(&installed.inspection.content_identity))
        {
            return Err(io::Error::other("desktop entry content identity is stale"));
        }
        let policy = read_policy()?.authority(
            session,
            &application,
            &installed.inspection.content_identity,
        )?;
        let user_root = Path::new(DATA).join("users").join(session.user().as_str());
        let args = request.args.iter().map(OsString::from).collect::<Vec<_>>();
        let principal = Principal::Application(application.id().clone());
        let selected = if let Some(file_request) = file_request {
            let file_entry = installed.inspection.manifest.entry.as_ref().and_then(|entry| entry.file_open.as_ref())
                .ok_or_else(|| io::Error::other("application does not declare file-open mode"))?;
            if !file_entry.mime_types.contains(&file_request.mime_type) { return Err(io::Error::other("handler does not accept the selected type")); }
            // Доступ к выбранному файлу проверяется до привилегированного закрепления пути.
            luna_security::authorize(&policy, EndpointRequests(vec![
                AuthorizationRequest { principal: principal.clone(), resource: Resource::Capability(SELECTED_CAPABILITY.into()), permission: Permission::Use },
                AuthorizationRequest { principal: principal.clone(), resource: Resource::FilesystemPath(SELECTED_LOGICAL.into()), permission: Permission::Read },
            ])).map_err(io::Error::other)?;
            Some(SelectedFile::inspect(&user_root.join("home"), &file_request.path()?, session.id(), self.credentials.uid)?)
        } else { None };
        let needs_endpoint = selected.is_none() && installed.inspection.manifest.capabilities.requested.iter().any(|cap| cap == OPEN_CAPABILITY);
        let application_endpoint = if needs_endpoint {
            luna_security::authorize(&policy, EndpointRequests(vec![
                AuthorizationRequest { principal: principal.clone(), resource: Resource::Capability(OPEN_CAPABILITY.into()), permission: Permission::Use },
                AuthorizationRequest { principal: principal.clone(), resource: Resource::FilesystemPath(APPLICATION_SOCKET.into()), permission: Permission::Read },
            ])).map_err(io::Error::other)?;
            Some(self.endpoint.clone())
        } else { None };
        let wayland = if installed
            .inspection
            .manifest
            .capabilities
            .requested
            .iter()
            .any(|cap| cap == WAYLAND_CAPABILITY)
        {
            let principal = Principal::Application(application.id().clone());
            // No privileged compositor connection before its resource grants.
            luna_security::authorize(
                &policy,
                EndpointRequests(vec![
                    AuthorizationRequest {
                        principal: principal.clone(),
                        resource: Resource::Capability(WAYLAND_CAPABILITY.into()),
                        permission: Permission::Use,
                    },
                    AuthorizationRequest {
                        principal,
                        resource: Resource::FilesystemPath(WAYLAND_LOGICAL.into()),
                        permission: Permission::Read,
                    },
                ]),
            )
            .map_err(io::Error::other)?;
            Some(self.wayland_endpoint(session)?)
        } else {
            None
        };
        let plan = authorize_installed_bundle_with_handoff(
            &installed,
            session,
            &user_root,
            &args,
            &policy,
            wayland.as_ref(),
            application_endpoint.as_ref(),
            selected.as_ref(),
        )
        .map_err(io::Error::other)?;
        // Authorization precedes mutable storage creation and materialization.
        prepare_storage(&user_root, application.id().as_str(), &self.credentials)?;
        let has_home = selected.is_none() && installed
            .inspection
            .manifest
            .capabilities
            .requested
            .iter()
            .any(|cap| cap == HOME_CAPABILITY);
        if has_home {
            let data = data_fd()?;
            let users = open_dir(Some(&data), "users", 0, false)?;
            let user = open_dir(
                Some(&users),
                &self.credentials.username,
                self.credentials.uid,
                false,
            )?;
            open_dir(Some(&user), "home", self.credentials.uid, false)?;
        }
        let mut environment: Vec<(OsString, OsString)> = [
            ("HOME", if has_home { HOME_LOGICAL } else { "/data" }),
            ("XDG_DATA_HOME", "/data"),
            ("XDG_CONFIG_HOME", "/config"),
            ("PATH", "/bin:/usr/bin"),
            ("LANG", "C.UTF-8"),
            ("USER", self.credentials.username.as_str()),
            ("LOGNAME", self.credentials.username.as_str()),
        ]
        .into_iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect();
        if wayland.is_some() {
            environment.push(("WAYLAND_DISPLAY".into(), WAYLAND_LOGICAL.into()));
            environment.push(("XDG_RUNTIME_DIR".into(), "/tmp".into()));
            environment.push(("GDK_BACKEND".into(), "wayland".into()));
            environment.push(("GSK_RENDERER".into(), "cairo".into()));
            environment.push(("XDG_DATA_DIRS".into(), "/usr/share".into()));
            environment.push(("XDG_CONFIG_DIRS".into(), "/etc".into()));
        }
        if installed
            .inspection
            .manifest
            .capabilities
            .requested
            .iter()
            .any(|cap| cap == TERMINAL_CAPABILITY)
        {
            environment.push(("SHELL".into(), "/bin/fish".into()));
            environment.push(("LIBGL_ALWAYS_SOFTWARE".into(), "1".into()));
            environment.push(("GALLIUM_DRIVER".into(), "llvmpipe".into()));
        }
        let mut context = ApplicationLaunchContext::new(
            LinuxMountNamespace,
            "/.luna/image",
            "/run/luna/app-instances",
        )
        .with_profile(RuntimeProfile::new("musl-bundle").map_err(io::Error::other)?)
        .with_credentials(self.credentials.clone(), self.groups.clone())
        .with_environment(environment)
        .with_trusted_source_root(&installed.payload_root)
        .with_trusted_source_root(user_root);
        if let Some(endpoint) = wayland {
            context = context
                .with_trusted_source_root(
                    endpoint.source().parent().expect("absolute socket parent"),
                )
                .with_wayland_endpoint(endpoint);
        }
        if let Some(endpoint) = application_endpoint {
            context = context.with_trusted_source_root("/run/luna").with_application_endpoint(endpoint);
        }
        if let Some(file) = selected { context = context.with_selected_file(file); }
        let id = self.apps.launch_authorized_plan(plan, session, runtime, &context).map_err(io::Error::other)?;
        self.launched.insert(id, LaunchedApplication {
            application,
            content_identity: installed.inspection.content_identity,
            can_open: needs_endpoint && has_home,
        });
        Ok(id)
    }

    fn wayland_endpoint(&self, session: &UserSession) -> io::Result<WaylandEndpoint> {
        let directory = std::path::PathBuf::from(format!("/run/user/{}", self.credentials.uid));
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.is_dir()
            || metadata.uid() != self.credentials.uid
            || metadata.mode() & 0o077 != 0
        {
            return Err(io::Error::other("unsafe session runtime directory"));
        }
        let mut candidates = Vec::new();
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(suffix) = name.to_str().and_then(|name| name.strip_prefix("wayland-")) else {
                continue;
            };
            if !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()) {
                candidates.push(entry.path());
            }
        }
        if candidates.len() != 1 {
            return Err(io::Error::other("no unique session Wayland endpoint"));
        }
        let endpoint =
            WaylandEndpoint::inspect(candidates.remove(0), session.id(), self.credentials.uid)?;
        let membership = fs::read_to_string(format!("/proc/{}/cgroup", endpoint.peer_pid()))?;
        if !luna_system_runtime::process_cgroup::member_of(&membership, &self.session_membership) {
            return Err(io::Error::other(
                "Wayland peer is outside the active desktop",
            ));
        }
        Ok(endpoint)
    }

    pub fn end_session(&mut self, runtime: &mut SystemRuntimeService) -> io::Result<()> {
        let mut failures = Vec::new();
        let ids = self.launched.keys().copied().collect::<Vec<_>>();
        for id in ids {
            if let Err(error) = self.apps.terminate(id, runtime) { failures.push(error.to_string()); continue; }
            self.launched.remove(&id);
            if let Err(error) = self.apps.forget_instance(id) { failures.push(error.to_string()); }
            for client in &mut self.clients {
                if client.instance == Some(id) {
                    client.instance = None;
                    let response = RunResponse::failed("UserSession ended; application terminated");
                    let _ = client.stream.write_all(toml::to_string(&response).map_err(io::Error::other)?.as_bytes());
                }
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(failures.join("; ")))
        }
    }
}
impl Drop for ApplicationBroker {
    fn drop(&mut self) {
        let _ = fs::remove_file(APPLICATION_SOCKET);
        let _ = fs::remove_dir("/run/luna/app-instances");
    }
}

fn pass_credentials(fd: i32) -> io::Result<()> {
    let enabled: libc::c_int = 1;
    // SAFETY: fd — живой сокет; указатель и размер соответствуют c_int.
    if unsafe { libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_PASSCRED,
        (&enabled as *const libc::c_int).cast(), std::mem::size_of_val(&enabled) as libc::socklen_t) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// SO_PEERCRED относится к создателю соединения; SO_PASSCRED проверяет каждый
/// фактический send. Передача сокета иному процессу не передаёт ему роль Files.
fn receive_request(stream: &UnixStream, buffer: &mut [u8], expected: &libc::ucred) -> io::Result<usize> {
    let mut control = [0usize; 64];
    let mut vector = libc::iovec { iov_base: buffer.as_mut_ptr().cast(), iov_len: buffer.len() };
    // SAFETY: нулевая msghdr допустима, все используемые указатели задаются ниже.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut vector; message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast(); message.msg_controllen = std::mem::size_of_val(&control) as _;
    // SAFETY: векторы и выровненный буфер живут до окончания recvmsg; CLOEXEC
    // защищает от наследования случайно полученных дескрипторов до их закрытия.
    let count = unsafe { libc::recvmsg(stream.as_raw_fd(), &mut message, libc::MSG_CMSG_CLOEXEC) };
    if count < 0 { return Err(io::Error::last_os_error()); }
    let mut credentials = None;
    let mut invalid = message.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC) != 0;
    // SAFETY: заголовки записаны ядром в указанный буфер. Перед чтением данных
    // проверяем длину; каждый полученный SCM_RIGHTS fd сразу получает владельца.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&message);
        while !header.is_null() {
            let item = &*header;
            let base = libc::CMSG_LEN(0) as usize;
            if (item.cmsg_len as usize) < base { invalid = true; break; }
            let bytes = item.cmsg_len as usize - base;
            let data = libc::CMSG_DATA(header);
            if item.cmsg_level == libc::SOL_SOCKET && item.cmsg_type == libc::SCM_CREDENTIALS
                && bytes == std::mem::size_of::<libc::ucred>() && credentials.is_none() {
                credentials = Some(std::ptr::read_unaligned(data.cast::<libc::ucred>()));
            } else if item.cmsg_level == libc::SOL_SOCKET && item.cmsg_type == libc::SCM_RIGHTS {
                for index in 0..bytes / std::mem::size_of::<i32>() {
                    let fd = std::ptr::read_unaligned(data.cast::<i32>().add(index));
                    if fd >= 0 { drop(OwnedFd::from_raw_fd(fd)); }
                }
                invalid = true;
            } else { invalid = true; }
            header = libc::CMSG_NXTHDR(&message, header);
        }
    }
    if count == 0 && !invalid { return Ok(0); }
    if invalid || !credentials.is_some_and(|peer: libc::ucred|
        peer.pid == expected.pid && peer.uid == expected.uid && peer.gid == expected.gid) {
        return Err(io::Error::other("request sender changed or ancillary data is unsupported"));
    }
    Ok(count as usize)
}

fn peer_credentials(stream: &UnixStream) -> io::Result<libc::ucred> {
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut size,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if size != std::mem::size_of::<libc::ucred>() as libc::socklen_t || credentials.pid <= 0 {
        return Err(io::Error::other("invalid peer credentials"));
    }
    Ok(credentials)
}

fn open_dir(
    parent: Option<&OwnedFd>,
    name: &str,
    owner: u32,
    private: bool,
) -> io::Result<OwnedFd> {
    let name = CString::new(name).map_err(io::Error::other)?;
    let fd = unsafe {
        libc::openat(
            parent.map_or(libc::AT_FDCWD, AsRawFd::as_raw_fd),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let metadata = File::from(fd.try_clone()?).metadata()?;
    if metadata.uid() != owner || metadata.mode() & if private { 0o077 } else { 0o022 } != 0 {
        return Err(io::Error::other(
            "unsafe application directory owner or mode",
        ));
    }
    Ok(fd)
}
fn data_fd() -> io::Result<OwnedFd> {
    let root = open_dir(None, "/", 0, false)?;
    let hidden = open_dir(Some(&root), ".luna", 0, false)?;
    open_dir(Some(&hidden), "data", 0, false)
}
fn read_policy() -> io::Result<LaunchPolicy> {
    let data = data_fd()?;
    let system = open_dir(Some(&data), "system", 0, false)?;
    let config = open_dir(Some(&system), "config", 0, false)?;
    let name = c"application-launch.toml";
    let fd = unsafe {
        libc::openat(
            config.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.len() > 65536
    {
        return Err(io::Error::other("unsafe application launch policy"));
    }
    let mut text = String::new();
    file.take(65537).read_to_string(&mut text)?;
    if text.len() > 65536 {
        return Err(io::Error::other("launch policy too large"));
    }
    toml::from_str(&text).map_err(io::Error::other)
}
fn prepare_storage(
    user_root: &Path,
    application: &str,
    credentials: &UserCredentials,
) -> io::Result<()> {
    let data = data_fd()?;
    let users = open_dir(Some(&data), "users", 0, false)?;
    let user = open_dir(Some(&users), &credentials.username, credentials.uid, false)?;
    for (logical, _) in private_paths(user_root, application) {
        let parent = open_dir(
            Some(&user),
            logical.trim_start_matches('/'),
            credentials.uid,
            false,
        )?;
        let name = CString::new(application).map_err(io::Error::other)?;
        // The verified manager identity is one path component. The directory
        // is initialized through a descriptor; existing data is never chowned.
        if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } == 0 {
            let fd = open_dir(Some(&parent), application, 0, true)?;
            if unsafe { libc::fchown(fd.as_raw_fd(), credentials.uid, credentials.gid) } != 0 {
                return Err(io::Error::last_os_error());
            }
        } else if io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists {
            return Err(io::Error::last_os_error());
        }
        open_dir(Some(&parent), application, credentials.uid, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_peers_cannot_consume_authenticated_capacity() {
        assert!(!client_capacity(true, 0, MAX_DENIED_CLIENTS));
        assert!(client_capacity(false, 0, MAX_DENIED_CLIENTS));
        assert!(client_capacity(false, MAX_CLIENTS - 1, MAX_DENIED_CLIENTS));
        assert!(!client_capacity(false, MAX_CLIENTS, 0));
        assert!(client_capacity(true, MAX_CLIENTS, MAX_DENIED_CLIENTS - 1));
    }
}

#[cfg(test)]
mod message_credentials_tests {
    use super::*;
    use std::net::Shutdown;
    #[test]
    fn listener_keeps_credentials_for_data_sent_before_accept() {
        let path = std::env::temp_dir().join(format!("luna-passcred-{}", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        pass_credentials(listener.as_raw_fd()).unwrap();
        let mut sender = UnixStream::connect(&path).unwrap();
        sender.write_all(b"request").unwrap(); sender.shutdown(Shutdown::Write).unwrap();
        let (receiver, _) = listener.accept().unwrap();
        let peer = peer_credentials(&receiver).unwrap(); let mut buffer = [0; 32];
        assert_eq!(receive_request(&receiver, &mut buffer, &peer).unwrap(), 7);
        assert_eq!(receive_request(&receiver, &mut buffer, &peer).unwrap(), 0);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn inherited_socket_cannot_change_request_sender() {
        let (receiver, sender) = UnixStream::pair().unwrap();
        pass_credentials(receiver.as_raw_fd()).unwrap();
        let peer = peer_credentials(&receiver).unwrap();
        // SAFETY: ребёнок вызывает только send/_exit без выделения памяти и Rust Drop.
        let child = unsafe { libc::fork() };
        assert!(child >= 0);
        if child == 0 {
            // SAFETY: сокет жив, постоянный буфер содержит ровно один байт.
            unsafe { libc::send(sender.as_raw_fd(), b"x".as_ptr().cast(), 1, libc::MSG_NOSIGNAL); libc::_exit(0); }
        }
        let mut buffer = [0; 32];
        assert!(receive_request(&receiver, &mut buffer, &peer).is_err());
        let mut status = 0;
        // SAFETY: ожидаем известного ребёнка, status — действительный указатель.
        assert_eq!(unsafe { libc::waitpid(child, &mut status, 0) }, child);
    }
    #[test]
    fn received_descriptors_are_rejected_and_closed() {
        let (receiver, sender) = UnixStream::pair().unwrap();
        pass_credentials(receiver.as_raw_fd()).unwrap();
        let peer = peer_credentials(&receiver).unwrap();
        let (payload, mut witness) = UnixStream::pair().unwrap();
        let mut control = [0usize; 8]; let mut byte = b'x';
        let mut vector = libc::iovec { iov_base: (&mut byte as *mut u8).cast(), iov_len: 1 };
        // SAFETY: msghdr и cmsg получают выровненный буфер указанной длины.
        unsafe {
            let mut message: libc::msghdr = std::mem::zeroed();
            message.msg_iov = &mut vector; message.msg_iovlen = 1;
            message.msg_control = control.as_mut_ptr().cast();
            message.msg_controllen = libc::CMSG_SPACE(std::mem::size_of::<i32>() as _) as _;
            let header = libc::CMSG_FIRSTHDR(&message);
            (*header).cmsg_level = libc::SOL_SOCKET; (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<i32>() as _) as _;
            std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<i32>(), payload.as_raw_fd());
            assert_eq!(libc::sendmsg(sender.as_raw_fd(), &message, 0), 1);
        }
        drop(payload);
        let mut buffer = [0; 32];
        assert!(receive_request(&receiver, &mut buffer, &peer).is_err());
        witness.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
        // EOF доказывает, что полученная копия fd не осталась открыта в посреднике.
        assert_eq!(witness.read(&mut buffer).unwrap(), 0);
    }
}
