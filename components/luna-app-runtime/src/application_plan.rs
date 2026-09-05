//! Explicit application execution plan and authorization boundary.
//!
//! This module turns a validated Bundle declaration plus an active UserSession
//! into an immutable execution plan. The plan can be authorized independently
//! from Linux namespace materialization and process creation.

use std::ffi::OsString;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use luna_bundle::{BundleKind, BundleManifest, ResourceAccess, validate_manifest};
use luna_common::{BundleId, RuntimeKind, RuntimeSpec, Version};
use luna_root_mapping::{LogicalPath, MappingError, MappingTable};
use luna_security::{
    AuthorizationRequest, Decision, Permission, PolicyAuthority, Principal, Resource,
    SecurityError,
};
use luna_user_session::{SessionId, SessionState, UserSession};

/// The executable identity that an application plan is permitted to launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableSpec {
    path: PathBuf,
    args: Vec<OsString>,
}

impl ExecutableSpec {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            args: Vec::new(),
        }
    }

    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn args(&self) -> &[OsString] {
        &self.args
    }
}

#[derive(Clone, Debug)]
pub struct ApplicationPlan {
    application: BundleId,
    version: Version,
    session: SessionId,
    runtime: RuntimeSpec,
    executable: ExecutableSpec,
    manifest: BundleManifest,
    mapping: MappingTable,
    requests: Vec<AuthorizationRequest>,
}

impl ApplicationPlan {
    pub fn new(
        manifest: BundleManifest,
        mapping: MappingTable,
        session: &UserSession,
        runtime: RuntimeSpec,
        executable: ExecutableSpec,
        requests: Vec<AuthorizationRequest>,
    ) -> Result<Self, PlanError> {
        let plan = Self {
            application: manifest.metadata().id().clone(),
            version: manifest.metadata().version(),
            session: session.id(),
            runtime,
            executable,
            manifest,
            mapping,
            requests,
        };
        plan.validate(session)?;
        Ok(plan)
    }

    pub fn validate(&self, session: &UserSession) -> Result<(), PlanError> {
        if self.manifest.metadata().kind() != BundleKind::Application {
            return Err(PlanError::NotApplicationBundle);
        }
        validate_manifest(&self.manifest)
            .map_err(|error| PlanError::InvalidBundle(error.to_string()))?;
        if session.id() != self.session || session.state() != SessionState::Active {
            return Err(PlanError::SessionNotActive(self.session));
        }
        if !self.mapping.accepts_runtime(self.runtime.kind()) {
            return Err(PlanError::RuntimeMismatch {
                mapping: self.mapping.runtime(),
                requested: self.runtime.kind(),
            });
        }
        Self::validate_executable(&self.executable, &self.manifest, &self.mapping)?;
        for resource in self.manifest.resources() {
            let logical = LogicalPath::new(resource.logical_path()).map_err(PlanError::Mapping)?;
            self.mapping.resolve(&logical).map_err(PlanError::Mapping)?;
        }
        for request in &self.requests {
            if request.principal != Principal::Application(self.application.clone()) {
                return Err(PlanError::ForeignPrincipal {
                    expected: self.application.clone(),
                });
            }
        }
        // `materialize` is deliberately a pure deterministic mapping operation;
        // it does not perform Linux namespace or mount operations.
        self.mapping.materialize().map_err(PlanError::Mapping)?;
        Ok(())
    }

    fn validate_executable(
        executable: &ExecutableSpec,
        manifest: &BundleManifest,
        mapping: &MappingTable,
    ) -> Result<(), PlanError> {
        let path = executable.path();
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
            || path.as_os_str().to_string_lossy().contains('\0')
            || path.to_str().is_none()
        {
            return Err(PlanError::InvalidExecutable(path.display().to_string()));
        }
        let logical = LogicalPath::new(path).map_err(PlanError::Mapping)?;
        mapping
            .resolve(&logical)
            .map_err(|_| PlanError::ExecutableNotMapped(path.display().to_string()))?;
        let resource = manifest
            .resources()
            .iter()
            .find(|resource| resource.logical_path() == path.to_string_lossy())
            .ok_or_else(|| PlanError::ExecutableNotDeclared(path.display().to_string()))?;
        if !resource.access().contains(&ResourceAccess::Execute) {
            return Err(PlanError::ExecutableNotExecutable(
                path.display().to_string(),
            ));
        }
        Ok(())
    }

    fn declared_requests(&self) -> Vec<AuthorizationRequest> {
        let principal = Principal::Application(self.application.clone());
        let mut requests = Vec::new();

        for resource in self.manifest.resources() {
            let resource_id = Resource::FilesystemPath(resource.logical_path().to_owned());
            for access in resource.access() {
                let permission = match access {
                    ResourceAccess::Read => Permission::Read,
                    ResourceAccess::Write => Permission::Write,
                    ResourceAccess::Execute => Permission::Execute,
                };
                requests.push(AuthorizationRequest {
                    principal: principal.clone(),
                    resource: resource_id.clone(),
                    permission,
                });
            }
        }

        for capability in self.manifest.capabilities() {
            requests.push(AuthorizationRequest {
                principal: principal.clone(),
                resource: Resource::Capability(capability.to_owned()),
                permission: Permission::Use,
            });
        }

        requests
    }

    pub fn authorize(
        self,
        policy: &dyn PolicyAuthority,
    ) -> Result<AuthorizedApplicationPlan, PlanError> {
        let runtime_request = AuthorizationRequest {
            principal: Principal::Application(self.application.clone()),
            resource: Resource::Runtime(self.runtime.kind()),
            permission: Permission::Use,
        };
        require_allow(policy, &runtime_request)?;
        for request in self.declared_requests() {
            require_allow(policy, &request)?;
        }
        for request in &self.requests {
            require_allow(policy, request)?;
        }
        Ok(AuthorizedApplicationPlan { plan: self })
    }

    pub fn application(&self) -> &BundleId {
        &self.application
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn session(&self) -> SessionId {
        self.session
    }

    pub fn runtime(&self) -> RuntimeSpec {
        self.runtime
    }

    pub fn executable(&self) -> &ExecutableSpec {
        &self.executable
    }

    pub fn manifest(&self) -> &BundleManifest {
        &self.manifest
    }

    pub fn mapping(&self) -> &MappingTable {
        &self.mapping
    }

    pub fn requests(&self) -> &[AuthorizationRequest] {
        &self.requests
    }
}

#[derive(Clone, Debug)]
pub struct AuthorizedApplicationPlan {
    plan: ApplicationPlan,
}

impl AuthorizedApplicationPlan {
    pub fn application(&self) -> &BundleId {
        self.plan.application()
    }

    pub fn version(&self) -> Version {
        self.plan.version()
    }

    pub fn session(&self) -> SessionId {
        self.plan.session()
    }

    pub fn runtime(&self) -> RuntimeSpec {
        self.plan.runtime()
    }

    pub fn executable(&self) -> &ExecutableSpec {
        self.plan.executable()
    }

    pub fn manifest(&self) -> &BundleManifest {
        self.plan.manifest()
    }

    pub fn mapping(&self) -> &MappingTable {
        self.plan.mapping()
    }

    pub fn requests(&self) -> &[AuthorizationRequest] {
        self.plan.requests()
    }

    pub fn into_plan(self) -> ApplicationPlan {
        self.plan
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanError {
    InvalidBundle(String),
    NotApplicationBundle,
    SessionNotActive(SessionId),
    Mapping(MappingError),
    InvalidExecutable(String),
    ExecutableNotMapped(String),
    ExecutableNotDeclared(String),
    ExecutableNotExecutable(String),
    RuntimeMismatch {
        mapping: Option<RuntimeKind>,
        requested: RuntimeKind,
    },
    ForeignPrincipal {
        expected: BundleId,
    },
    Security(String),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBundle(error) => write!(f, "invalid bundle: {error}"),
            Self::NotApplicationBundle => {
                f.write_str("execution plan requires an application bundle")
            }
            Self::SessionNotActive(id) => write!(f, "session {} is not active", id.get()),
            Self::Mapping(error) => write!(f, "mapping error: {error}"),
            Self::InvalidExecutable(path) => write!(f, "invalid executable: {path}"),
            Self::ExecutableNotMapped(path) => write!(f, "executable is not mapped: {path}"),
            Self::ExecutableNotDeclared(path) => {
                write!(f, "executable is not declared as a bundle resource: {path}")
            }
            Self::ExecutableNotExecutable(path) => {
                write!(f, "executable resource lacks execute access: {path}")
            }
            Self::RuntimeMismatch { mapping, requested } => write!(
                f,
                "runtime mismatch: mapping={mapping:?}, requested={requested}"
            ),
            Self::ForeignPrincipal { expected } => write!(
                f,
                "authorization request principal does not match application {expected}"
            ),
            Self::Security(error) => write!(f, "authorization failed: {error}"),
        }
    }
}

impl std::error::Error for PlanError {}

fn require_allow(
    policy: &dyn PolicyAuthority,
    request: &AuthorizationRequest,
) -> Result<(), PlanError> {
    match policy
        .authorize(request)
        .map_err(|error: SecurityError| PlanError::Security(error.to_string()))?
    {
        Decision::Allow => Ok(()),
        Decision::Deny => Err(PlanError::Security(format!("denied: {request:?}"))),
        Decision::Ask => Err(PlanError::Security(
            "authorization requires explicit user confirmation".into(),
        )),
        Decision::Constrained { constraints } => Err(PlanError::Security(format!(
            "constraint enforcement is not part of this plan boundary: {constraints:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{ApplicationPlan, ExecutableSpec, PlanError};
    use luna_bundle::{BundleKind, BundleManifest, BundleMetadata, BundleResource, ResourceAccess};
    use luna_common::{BundleId, RuntimeSpec, UserId, Version};
    use luna_root_mapping::{LogicalPath, MappingRule, MappingTable, PhysicalPath};
    use luna_security::{
        AuthorizationRequest, Constraint, Decision, Permission, PolicyAuthority, Principal,
        Resource, SecurityError,
    };
    use luna_user_session::{SessionId, SessionState, UserSession};

    fn active_session() -> UserSession {
        let mut session = UserSession::new(SessionId::new(7), UserId::from("alice"));
        session
            .transition(SessionState::Authenticating)
            .expect("authenticate transition");
        session.login_succeeded().expect("active session");
        session
    }

    fn valid_manifest() -> BundleManifest {
        let mut manifest = BundleManifest::new(BundleMetadata::new(
            BundleId::from("example.app"),
            Version::new(1, 0, 0),
            BundleKind::Application,
        ));
        manifest.add_resource(
            BundleResource::new("/bin/app", "bin/app")
                .with_access([ResourceAccess::Execute]),
        );
        manifest
    }

    fn valid_mapping() -> MappingTable {
        let logical = LogicalPath::new("/bin/app").unwrap();
        let mut mapping = MappingTable::new();
        mapping
            .insert(MappingRule::file(
                logical,
                PhysicalPath::new("/data/apps/example/bin/app"),
            ))
            .unwrap();
        mapping
    }

    fn plan() -> ApplicationPlan {
        ApplicationPlan::new(
            valid_manifest(),
            valid_mapping(),
            &active_session(),
            RuntimeSpec::luna(),
            ExecutableSpec::new("/bin/app"),
            vec![],
        )
        .unwrap()
    }

    struct Deny;
    impl PolicyAuthority for Deny {
        fn authorize(&self, _request: &AuthorizationRequest) -> Result<Decision, SecurityError> {
            Ok(Decision::Deny)
        }
    }

    struct Allow;
    impl PolicyAuthority for Allow {
        fn authorize(&self, _request: &AuthorizationRequest) -> Result<Decision, SecurityError> {
            Ok(Decision::Allow)
        }
    }

    #[test]
    fn plan_requires_active_bound_session() {
        let session = UserSession::new(SessionId::new(7), UserId::from("alice"));
        let result = ApplicationPlan::new(
            valid_manifest(),
            valid_mapping(),
            &session,
            RuntimeSpec::luna(),
            ExecutableSpec::new("/bin/app"),
            vec![],
        );
        assert!(matches!(result, Err(PlanError::SessionNotActive(_))));
    }

    #[test]
    fn executable_must_be_part_of_mapping() {
        let result = ApplicationPlan::new(
            valid_manifest(),
            valid_mapping(),
            &active_session(),
            RuntimeSpec::luna(),
            ExecutableSpec::new("/bin/not-mapped"),
            vec![],
        );
        assert!(matches!(
            result,
            Err(PlanError::ExecutableNotMapped(path)) if path == "/bin/not-mapped"
        ));
    }

    #[test]
    fn executable_requires_execute_access() {
        let mut manifest = BundleManifest::new(BundleMetadata::new(
            BundleId::from("example.app"),
            Version::new(1, 0, 0),
            BundleKind::Application,
        ));
        manifest.add_resource(BundleResource::new("/bin/app", "bin/app"));
        let result = ApplicationPlan::new(
            manifest,
            valid_mapping(),
            &active_session(),
            RuntimeSpec::luna(),
            ExecutableSpec::new("/bin/app"),
            vec![],
        );
        assert!(matches!(
            result,
            Err(PlanError::ExecutableNotExecutable(path)) if path == "/bin/app"
        ));
    }

    #[test]
    fn executable_must_be_normalized() {
        let result = ApplicationPlan::new(
            valid_manifest(),
            valid_mapping(),
            &active_session(),
            RuntimeSpec::luna(),
            ExecutableSpec::new("/bin/./app"),
            vec![],
        );
        assert!(matches!(result, Err(PlanError::InvalidExecutable(_))));

        let result = ApplicationPlan::new(
            valid_manifest(),
            valid_mapping(),
            &active_session(),
            RuntimeSpec::luna(),
            ExecutableSpec::new("/bin/../bin/app"),
            vec![],
        );
        assert!(matches!(result, Err(PlanError::InvalidExecutable(_))));
    }

    #[test]
    fn runtime_mismatch_is_rejected_before_authorization() {
        let mut mapping = valid_mapping();
        mapping
            .bind_runtime(luna_common::RuntimeKind::Glibc)
            .unwrap();
        let result = ApplicationPlan::new(
            valid_manifest(),
            mapping,
            &active_session(),
            RuntimeSpec::luna(),
            ExecutableSpec::new("/bin/app"),
            vec![],
        );
        assert!(matches!(result, Err(PlanError::RuntimeMismatch { .. })));
    }

    #[test]
    fn authorization_is_fail_closed() {
        let result = plan().authorize(&Deny);
        assert!(matches!(result, Err(PlanError::Security(_))));
    }

    #[test]
    fn declared_filesystem_access_and_capabilities_are_authorized() {
        let mut manifest = valid_manifest();
        manifest.add_resource(
            BundleResource::new("/home/alice/config", "config")
                .with_access([ResourceAccess::Read, ResourceAccess::Write]),
        );
        manifest.add_capability("network");
        let plan = ApplicationPlan::new(
            manifest,
            valid_mapping(),
            &active_session(),
            RuntimeSpec::luna(),
            ExecutableSpec::new("/bin/app"),
            vec![],
        )
        .unwrap();
        assert!(plan.clone().authorize(&Deny).is_err());

        struct RecordingAllow {
            seen: std::cell::RefCell<Vec<AuthorizationRequest>>,
        }
        impl PolicyAuthority for RecordingAllow {
            fn authorize(
                &self,
                request: &AuthorizationRequest,
            ) -> Result<Decision, SecurityError> {
                self.seen.borrow_mut().push(request.clone());
                Ok(Decision::Allow)
            }
        }

        let policy = RecordingAllow {
            seen: std::cell::RefCell::new(Vec::new()),
        };
        plan.authorize(&policy).unwrap();
        let seen = policy.seen.borrow();
        assert!(seen.iter().any(|request| {
            request.resource == Resource::FilesystemPath("/home/alice/config".into())
                && request.permission == Permission::Read
        }));
        assert!(seen.iter().any(|request| {
            request.resource == Resource::FilesystemPath("/home/alice/config".into())
                && request.permission == Permission::Write
        }));
        assert!(seen.iter().any(|request| {
            request.resource == Resource::Capability("network".into())
                && request.permission == Permission::Use
        }));
    }

    #[test]
    fn authorization_requires_runtime_permission() {
        struct RuntimeDeny;
        impl PolicyAuthority for RuntimeDeny {
            fn authorize(
                &self,
                request: &AuthorizationRequest,
            ) -> Result<Decision, SecurityError> {
                if matches!(request.resource, Resource::Runtime(_)) {
                    Ok(Decision::Deny)
                } else {
                    Ok(Decision::Allow)
                }
            }
        }

        assert!(plan().authorize(&RuntimeDeny).is_err());
    }

    #[test]
    fn authorization_rejects_ask_and_constraints() {
        struct Ask;
        impl PolicyAuthority for Ask {
            fn authorize(
                &self,
                _request: &AuthorizationRequest,
            ) -> Result<Decision, SecurityError> {
                Ok(Decision::Ask)
            }
        }

        struct Constrained;
        impl PolicyAuthority for Constrained {
            fn authorize(
                &self,
                _request: &AuthorizationRequest,
            ) -> Result<Decision, SecurityError> {
                Ok(Decision::Constrained {
                    constraints: vec![Constraint::ReadOnly],
                })
            }
        }

        assert!(plan().authorize(&Ask).is_err());
        assert!(plan().authorize(&Constrained).is_err());
    }

    #[test]
    fn successful_authorization_returns_owned_authorized_plan() {
        let authorized = plan().authorize(&Allow).unwrap();
        assert_eq!(authorized.application().as_str(), "example.app");
        assert_eq!(authorized.runtime(), RuntimeSpec::luna());
        assert_eq!(authorized.executable().path().to_str().unwrap(), "/bin/app");
    }

    #[test]
    fn request_principal_is_bound_to_application_identity() {
        let request = AuthorizationRequest {
            principal: Principal::Application(BundleId::from("other.app")),
            resource: Resource::UserData(UserId::from("alice")),
            permission: Permission::Read,
        };
        let result = ApplicationPlan::new(
            valid_manifest(),
            valid_mapping(),
            &active_session(),
            RuntimeSpec::luna(),
            ExecutableSpec::new("/bin/app"),
            vec![request],
        );
        assert!(matches!(result, Err(PlanError::ForeignPrincipal { .. })));
    }
}
