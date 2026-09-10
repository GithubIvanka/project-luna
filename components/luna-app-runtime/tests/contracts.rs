use luna_app_runtime::{
    ApplicationInstance, ApplicationInstanceId, ApplicationPlan, ApplicationRuntime,
    ExecutableSpec,
};
use luna_bundle::{
    BundleKind, BundleManifest, BundleMetadata, BundleResource, ResourceAccess,
};
use luna_common::{BundleId, RuntimeSpec, UserId, Version};
use luna_root_mapping::{LogicalPath, MappingRule, MappingTable, PhysicalPath};
use luna_security::{
    AuthorizationRequest, Decision, PolicyAuthority, SecurityError,
};
use luna_user_session::{SessionId, SessionState, UserSession};

struct AllowAll;

impl PolicyAuthority for AllowAll {
    fn authorize(&self, _request: &AuthorizationRequest) -> Result<Decision, SecurityError> {
        Ok(Decision::Allow)
    }
}

struct DenyAll;

impl PolicyAuthority for DenyAll {
    fn authorize(&self, _request: &AuthorizationRequest) -> Result<Decision, SecurityError> {
        Ok(Decision::Deny)
    }
}

#[derive(Default)]
struct TestRuntime {
    launches: usize,
}

impl ApplicationRuntime for TestRuntime {
    type Error = &'static str;

    fn launch_authorized(
        &mut self,
        plan: luna_app_runtime::AuthorizedApplicationPlan,
        session: &UserSession,
    ) -> Result<ApplicationInstance, Self::Error> {
        if session.id() != plan.session() || session.state() != SessionState::Active {
            return Err("invalid launch session");
        }
        self.launches += 1;
        Ok(ApplicationInstance::new_with_runtime(
            ApplicationInstanceId::new(1),
            plan.application().clone(),
            plan.version(),
            plan.session(),
            plan.runtime(),
        ))
    }

    fn authorize(
        &self,
        policy: &dyn PolicyAuthority,
        request: &AuthorizationRequest,
    ) -> Result<Decision, Self::Error> {
        policy.authorize(request).map_err(|_| "policy failure")
    }
}

fn active_session(id: u128) -> UserSession {
    let mut session = UserSession::new(SessionId::new(id), UserId::from("alice"));
    session
        .transition(SessionState::Authenticating)
        .expect("authenticate transition");
    session.login_succeeded().expect("active session");
    session
}

fn plan(session: &UserSession) -> ApplicationPlan {
    let metadata = BundleMetadata::new(
        BundleId::from("example.app"),
        Version::new(1, 0, 0),
        BundleKind::Application,
    );
    let mut manifest = BundleManifest::new(metadata);
    manifest.add_resource(
        BundleResource::new("/bin/app", "resources/bin/app")
            .with_access([ResourceAccess::Execute]),
    );

    let logical = LogicalPath::new("/bin/app").expect("logical path");
    let mut mapping = MappingTable::new();
    mapping
        .insert(
            MappingRule::file(
                logical,
                PhysicalPath::new("/data/system/apps/example/resources/bin/app"),
            )
            .with_access([ResourceAccess::Execute]),
        )
        .expect("mapping");

    ApplicationPlan::new(
        manifest,
        mapping,
        session,
        RuntimeSpec::luna(),
        ExecutableSpec::new("/bin/app"),
        Vec::new(),
    )
    .expect("valid application plan")
}

#[test]
fn runtime_boundary_accepts_only_authorized_plan_for_active_session() {
    let session = active_session(1);
    let authorized = plan(&session).authorize(&AllowAll).expect("authorization");
    let mut runtime = TestRuntime::default();

    let instance = runtime
        .launch_authorized(authorized, &session)
        .expect("authorized launch");

    assert_eq!(instance.application().as_str(), "example.app");
    assert_eq!(instance.session(), session.id());
    assert_eq!(runtime.launches, 1);
}

#[test]
fn authorization_denial_never_reaches_process_launch_boundary() {
    let session = active_session(2);
    let mut runtime = TestRuntime::default();

    assert!(plan(&session).authorize(&DenyAll).is_err());
    assert_eq!(runtime.launches, 0);

    let request = AuthorizationRequest {
        principal: luna_security::Principal::Application(BundleId::from("example.app")),
        resource: luna_security::Resource::Runtime(luna_common::RuntimeKind::Luna),
        permission: luna_security::Permission::Use,
    };
    assert_eq!(runtime.authorize(&AllowAll, &request).unwrap(), Decision::Allow);
}

#[test]
fn inactive_or_foreign_session_is_rejected_at_runtime_boundary() {
    let authorized_session = active_session(3);
    let authorized = plan(&authorized_session)
        .authorize(&AllowAll)
        .expect("authorization");
    let inactive = UserSession::new(SessionId::new(3), UserId::from("alice"));
    let mut runtime = TestRuntime::default();
    assert!(runtime.launch_authorized(authorized, &inactive).is_err());
    assert_eq!(runtime.launches, 0);

    let authorized = plan(&authorized_session)
        .authorize(&AllowAll)
        .expect("authorization");
    let foreign = active_session(4);
    assert!(runtime.launch_authorized(authorized, &foreign).is_err());
    assert_eq!(runtime.launches, 0);
}
