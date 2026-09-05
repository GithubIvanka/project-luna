use std::cell::RefCell;

use luna_app_runtime::{ApplicationPlan, ExecutableSpec, PlanError};
use luna_bundle::{BundleKind, BundleManifest, BundleMetadata, BundleResource};
use luna_common::{BundleId, ResourceAccess, RuntimeSpec, UserId, Version};
use luna_root_mapping::{LogicalPath, MappingRule, MappingTable, PhysicalPath};
use luna_security::{
    AuthorizationRequest, Decision, Permission, PolicyAuthority, Principal, Resource, SecurityError,
};
use luna_user_session::{SessionId, SessionState, UserSession};

fn active_session() -> UserSession {
    let mut session = UserSession::new(SessionId::new(9), UserId::from("alice"));
    session
        .transition(SessionState::Authenticating)
        .expect("authenticate transition");
    session.login_succeeded().expect("active session");
    session
}

fn plan(requests: Vec<AuthorizationRequest>) -> ApplicationPlan {
    let mut manifest = BundleManifest::new(BundleMetadata::new(
        BundleId::from("example.app"),
        Version::new(1, 0, 0),
        BundleKind::Application,
    ));
    manifest.add_resource(
        BundleResource::new("/bin/app", "bin/app").with_access([ResourceAccess::Execute]),
    );

    let mut mapping = MappingTable::new();
    mapping
        .insert(
            MappingRule::file(
                LogicalPath::new("/bin/app").unwrap(),
                PhysicalPath::new("/data/apps/example/bin/app"),
            )
            .with_access([ResourceAccess::Execute]),
        )
        .unwrap();

    ApplicationPlan::new(
        manifest,
        mapping,
        &active_session(),
        RuntimeSpec::luna(),
        ExecutableSpec::new("/bin/app"),
        requests,
    )
    .unwrap()
}

struct RecordingPolicy {
    decisions: Vec<Decision>,
    seen: RefCell<Vec<AuthorizationRequest>>,
}

impl RecordingPolicy {
    fn new(decisions: Vec<Decision>) -> Self {
        Self {
            decisions,
            seen: RefCell::new(Vec::new()),
        }
    }
}

impl PolicyAuthority for RecordingPolicy {
    fn authorize(&self, request: &AuthorizationRequest) -> Result<Decision, SecurityError> {
        self.seen.borrow_mut().push(request.clone());
        let index = self.seen.borrow().len() - 1;
        Ok(self.decisions.get(index).cloned().unwrap_or(Decision::Deny))
    }
}

#[test]
fn runtime_authorization_precedes_explicit_resource_requests() {
    let explicit = AuthorizationRequest {
        principal: Principal::Application(BundleId::from("example.app")),
        resource: Resource::UserData(UserId::from("alice")),
        permission: Permission::Read,
    };
    let policy = RecordingPolicy::new(vec![Decision::Allow, Decision::Allow]);

    let authorized = plan(vec![explicit.clone()]).authorize(&policy).unwrap();

    assert_eq!(authorized.application().as_str(), "example.app");
    let seen = policy.seen.borrow();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        seen[0].resource,
        Resource::Runtime(luna_common::RuntimeKind::Luna)
    );
    assert_eq!(seen[0].permission, Permission::Use);
    assert_eq!(seen[1], explicit);
}

#[test]
fn explicit_denial_stops_authorization_pipeline() {
    let first = AuthorizationRequest {
        principal: Principal::Application(BundleId::from("example.app")),
        resource: Resource::UserData(UserId::from("alice")),
        permission: Permission::Read,
    };
    let second = AuthorizationRequest {
        principal: Principal::Application(BundleId::from("example.app")),
        resource: Resource::ApplicationData {
            user: UserId::from("alice"),
            application: BundleId::from("example.app"),
        },
        permission: Permission::Write,
    };
    let policy = RecordingPolicy::new(vec![Decision::Allow, Decision::Deny, Decision::Allow]);

    let result = plan(vec![first, second]).authorize(&policy);

    assert!(matches!(result, Err(PlanError::Security(_))));
    let seen = policy.seen.borrow();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        seen[0].resource,
        Resource::Runtime(luna_common::RuntimeKind::Luna)
    );
    assert_eq!(seen[1].resource, Resource::UserData(UserId::from("alice")));
}
