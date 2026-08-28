use luna_app_runtime::{ApplicationInstance, ApplicationInstanceId, ApplicationRuntime};
use luna_bundle::{BundleKind, BundleManifest, BundleMetadata, BundleResource};
use luna_common::{BundleId, Version};
use luna_root_mapping::{LogicalPath, MappingRule, MappingTable, PhysicalPath};
use luna_security::{AuthorizationRequest, Decision, Permission, PolicyAuthority, Principal, Resource};
use luna_user_session::SessionId;

struct AllowAll;

impl PolicyAuthority for AllowAll {
    fn authorize(
        &self,
        _request: &AuthorizationRequest,
    ) -> Result<Decision, luna_security::SecurityError> {
        Ok(Decision::Allow)
    }
}

struct TestRuntime;

impl ApplicationRuntime for TestRuntime {
    type Error = &'static str;

    fn launch(
        &mut self,
        manifest: &BundleManifest,
        _mapping: &MappingTable,
    ) -> Result<ApplicationInstance, Self::Error> {
        Ok(ApplicationInstance::new(
            ApplicationInstanceId::new(1),
            manifest.metadata().id().clone(),
            manifest.metadata().version(),
            SessionId::new(1),
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

#[test]
fn runtime_boundary_composes_bundle_mapping_and_security_contracts() {
    let metadata = BundleMetadata::new(
        BundleId::from("example.app"),
        Version::new(1, 0, 0),
        BundleKind::Application,
    );
    let mut manifest = BundleManifest::new(metadata);
    manifest.add_resource(BundleResource::new("/bin/app", "resources/bin/app"));

    let logical = LogicalPath::new("/bin/app").expect("logical path");
    let mut mapping = MappingTable::new();
    mapping
        .insert(MappingRule::new(
            logical,
            PhysicalPath::new("/data/system/apps/example/resources/bin/app"),
        ))
        .expect("mapping");

    let mut runtime = TestRuntime;
    let instance = runtime.launch(&manifest, &mapping).expect("launch");
    assert_eq!(instance.application().as_str(), "example.app");

    let request = AuthorizationRequest {
        principal: Principal::Application(BundleId::from("example.app")),
        resource: Resource::UserData(luna_common::UserId::from("alice")),
        permission: Permission::Read,
    };
    assert_eq!(runtime.authorize(&AllowAll, &request).expect("authorize"), Decision::Allow);
}
