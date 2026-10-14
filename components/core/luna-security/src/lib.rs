//! Central security-policy authority for Project Luna.
//!
//! This crate evaluates policy. Low-level kernel/filesystem primitives remain
//! responsible for enforcing the result.

use luna_common::{BundleId, RuntimeKind, UserId, VolumePath};
use std::collections::BTreeSet;
use std::fmt;

mod capability;

pub use capability::{CapabilityGrant, CapabilityName, CapabilityProvider, CapabilityRegistry};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Principal {
    User(UserId),
    Application(BundleId),
    System,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Resource {
    UserData(UserId),
    ApplicationData {
        user: UserId,
        application: BundleId,
    },
    Application(BundleId),
    /// Trust in exact Bundle content for an operation; never an execution grant.
    BundleTrust {
        application: BundleId,
        content_identity: [u8; 32],
        scope: BundleTrustScope,
    },
    FilesystemPath(String),
    Capability(String),
    Runtime(RuntimeKind),
    /// Whole-volume management/visibility resource, not a path grant.
    Volume(String),
    /// A grant for one exact storage path.
    VolumePath(VolumePath),
    /// A grant to a directory and its descendants. It is distinct from an
    /// exact-path grant so mapping a subtree cannot silently broaden access.
    VolumeSubtree(VolumePath),
    Device(String),
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum BundleTrustScope {
    Install,
    Launch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Permission {
    Visibility,
    Read,
    Write,
    Execute,
    Use,
    Manage,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Constraint {
    ReadOnly,
    PathLimited(String),
    DeviceLimited(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision {
    Allow,
    Deny,
    Ask,
    Constrained { constraints: Vec<Constraint> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationRequest {
    pub principal: Principal,
    pub resource: Resource,
    pub permission: Permission,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityError(String);
impl SecurityError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SecurityError {}

pub trait PolicyAuthority {
    fn authorize(&self, request: &AuthorizationRequest) -> Result<Decision, SecurityError>;
}

/// Supplies the complete request set for one security decision.
pub trait AuthorizationSource {
    fn authorization_requests(&self) -> Vec<AuthorizationRequest>;
}

/// A value sealed by `luna-security` after every request was allowed.
#[derive(Clone, Debug)]
pub struct Authorized<T> {
    value: T,
}

impl<T> Authorized<T> {
    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn into_value(self) -> T {
        self.value
    }
}

/// The only constructor for `Authorized<T>`.
pub fn authorize<T: AuthorizationSource>(
    policy: &dyn PolicyAuthority,
    value: T,
) -> Result<Authorized<T>, SecurityError> {
    for request in value.authorization_requests() {
        match policy.authorize(&request)? {
            Decision::Allow => {}
            Decision::Deny => return Err(SecurityError::new(format!("denied: {request:?}"))),
            Decision::Ask => {
                return Err(SecurityError::new(
                    "authorization requires explicit user confirmation",
                ));
            }
            Decision::Constrained { constraints } => {
                return Err(SecurityError::new(format!(
                    "constraint enforcement is not available: {constraints:?}"
                )));
            }
        }
    }
    Ok(Authorized { value })
}

#[derive(Clone, Debug, Default)]
pub struct StaticPolicyAuthority {
    grants: BTreeSet<(Principal, Resource, Permission)>,
}
impl StaticPolicyAuthority {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn grant(&mut self, principal: Principal, resource: Resource, permission: Permission) {
        self.grants.insert((principal, resource, permission));
    }
    pub fn revoke(&mut self, principal: &Principal, resource: &Resource, permission: Permission) {
        self.grants
            .remove(&(principal.clone(), resource.clone(), permission));
    }
    pub fn is_granted(&self, request: &AuthorizationRequest) -> bool {
        if self.grants.contains(&(
            request.principal.clone(),
            request.resource.clone(),
            request.permission,
        )) {
            return true;
        }

        self.grants.iter().any(|(principal, resource, permission)| {
            if principal != &request.principal || permission != &request.permission {
                return false;
            }
            let Resource::VolumeSubtree(scope) = resource else {
                return false;
            };
            match &request.resource {
                Resource::VolumePath(path) | Resource::VolumeSubtree(path) => {
                    path_is_within_volume_scope(path, scope)
                }
                _ => false,
            }
        })
    }
}
fn path_is_within_volume_scope(path: &VolumePath, scope: &VolumePath) -> bool {
    if path.volume() != scope.volume() {
        return false;
    }
    let path = path.relative_path();
    let root = scope.relative_path().trim_end_matches('/');
    root.is_empty()
        || root == "/"
        || path == root
        || path
            .strip_prefix(root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

impl PolicyAuthority for StaticPolicyAuthority {
    fn authorize(&self, request: &AuthorizationRequest) -> Result<Decision, SecurityError> {
        Ok(if self.is_granted(request) {
            Decision::Allow
        } else {
            Decision::Deny
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorizationRequest, Constraint, Decision, Permission, PolicyAuthority, Principal,
        Resource, StaticPolicyAuthority,
    };
    use luna_common::{BundleId, RuntimeKind, UserId, VolumePath};

    #[test]
    fn explicit_grant_allows_and_revoke_denies() {
        let principal = Principal::Application(BundleId::from("example.app"));
        let resource = Resource::UserData(UserId::from("alice"));
        let request = AuthorizationRequest {
            principal: principal.clone(),
            resource: resource.clone(),
            permission: Permission::Read,
        };
        let mut policy = StaticPolicyAuthority::new();
        assert_eq!(policy.authorize(&request).unwrap(), Decision::Deny);
        policy.grant(principal.clone(), resource.clone(), Permission::Read);
        assert_eq!(policy.authorize(&request).unwrap(), Decision::Allow);
        policy.revoke(&principal, &resource, Permission::Read);
        assert_eq!(policy.authorize(&request).unwrap(), Decision::Deny);
    }

    #[test]
    fn visibility_is_independent_from_read() {
        let principal = Principal::Application(BundleId::from("example.app"));
        let resource = Resource::Application(BundleId::from("example.app"));
        let mut policy = StaticPolicyAuthority::new();
        let visible = AuthorizationRequest {
            principal: principal.clone(),
            resource: resource.clone(),
            permission: Permission::Visibility,
        };
        let readable = AuthorizationRequest {
            principal,
            resource,
            permission: Permission::Read,
        };
        policy.grant(
            visible.principal.clone(),
            visible.resource.clone(),
            Permission::Visibility,
        );
        assert_eq!(policy.authorize(&visible).unwrap(), Decision::Allow);
        assert_eq!(policy.authorize(&readable).unwrap(), Decision::Deny);
    }

    #[test]
    fn grants_are_scoped_to_principal_resource_and_permission() {
        let principal = Principal::Application(BundleId::from("example.app"));
        let other_principal = Principal::Application(BundleId::from("other.app"));
        let resource = Resource::Application(BundleId::from("example.app"));
        let other_resource = Resource::Application(BundleId::from("other.app"));
        let read = AuthorizationRequest {
            principal: principal.clone(),
            resource: resource.clone(),
            permission: Permission::Read,
        };
        let write = AuthorizationRequest {
            principal: principal.clone(),
            resource: resource.clone(),
            permission: Permission::Write,
        };
        let other_principal_request = AuthorizationRequest {
            principal: other_principal,
            resource: resource.clone(),
            permission: Permission::Read,
        };
        let other_resource_request = AuthorizationRequest {
            principal,
            resource: other_resource,
            permission: Permission::Read,
        };
        let mut policy = StaticPolicyAuthority::new();
        policy.grant(
            read.principal.clone(),
            read.resource.clone(),
            Permission::Read,
        );
        assert_eq!(policy.authorize(&read).unwrap(), Decision::Allow);
        assert_eq!(policy.authorize(&write).unwrap(), Decision::Deny);
        assert_eq!(
            policy.authorize(&other_principal_request).unwrap(),
            Decision::Deny
        );
        assert_eq!(
            policy.authorize(&other_resource_request).unwrap(),
            Decision::Deny
        );
    }

    #[test]
    fn runtime_requires_explicit_use_permission() {
        let principal = Principal::Application(BundleId::from("example.app"));
        let resource = Resource::Runtime(RuntimeKind::Glibc);
        let request = AuthorizationRequest {
            principal: principal.clone(),
            resource: resource.clone(),
            permission: Permission::Use,
        };
        let mut policy = StaticPolicyAuthority::new();
        assert_eq!(policy.authorize(&request).unwrap(), Decision::Deny);
        policy.grant(principal, resource, Permission::Use);
        assert_eq!(policy.authorize(&request).unwrap(), Decision::Allow);
    }

    #[test]
    fn filesystem_and_named_capabilities_are_distinct_resources() {
        let principal = Principal::Application(BundleId::from("example.app"));
        let filesystem = Resource::FilesystemPath("/home/alice/Documents".into());
        let network = Resource::Capability("network".into());
        let mut policy = StaticPolicyAuthority::new();
        let read_request = AuthorizationRequest {
            principal: principal.clone(),
            resource: filesystem,
            permission: Permission::Read,
        };
        let network_request = AuthorizationRequest {
            principal: principal.clone(),
            resource: network,
            permission: Permission::Use,
        };
        policy.grant(
            read_request.principal.clone(),
            read_request.resource.clone(),
            Permission::Read,
        );
        assert_eq!(policy.authorize(&read_request).unwrap(), Decision::Allow);
        assert_eq!(policy.authorize(&network_request).unwrap(), Decision::Deny);
    }

    #[test]
    fn volume_path_grants_are_scoped_to_volume_and_relative_path() {
        let principal = Principal::Application(BundleId::from("example.app"));
        let volume =
            luna_common::VolumeRef::new(luna_common::VolumeId::from("partition-guid:data"));
        let other_volume =
            luna_common::VolumeRef::new(luna_common::VolumeId::from("partition-guid:usb"));
        let requested =
            Resource::VolumePath(VolumePath::new(volume.clone(), "/Documents/readme.txt").unwrap());
        let sibling =
            Resource::VolumePath(VolumePath::new(volume, "/Documents/secret.txt").unwrap());
        let other_volume_path =
            Resource::VolumePath(VolumePath::new(other_volume, "/Documents/readme.txt").unwrap());
        let mut policy = StaticPolicyAuthority::new();
        let request = AuthorizationRequest {
            principal: principal.clone(),
            resource: requested.clone(),
            permission: Permission::Read,
        };
        policy.grant(principal.clone(), requested, Permission::Read);

        assert_eq!(policy.authorize(&request).unwrap(), Decision::Allow);
        assert_eq!(
            policy
                .authorize(&AuthorizationRequest {
                    principal: principal.clone(),
                    resource: sibling,
                    permission: Permission::Read,
                })
                .unwrap(),
            Decision::Deny
        );
        assert_eq!(
            policy
                .authorize(&AuthorizationRequest {
                    principal,
                    resource: other_volume_path,
                    permission: Permission::Read,
                })
                .unwrap(),
            Decision::Deny
        );
    }

    #[test]
    fn volume_subtree_grants_cover_descendants_only_on_the_same_volume() {
        let principal = Principal::Application(BundleId::from("example.app"));
        let volume =
            luna_common::VolumeRef::new(luna_common::VolumeId::from("partition-guid:data"));
        let other_volume =
            luna_common::VolumeRef::new(luna_common::VolumeId::from("partition-guid:usb"));
        let scope = Resource::VolumeSubtree(VolumePath::new(volume.clone(), "/Documents").unwrap());
        let child =
            Resource::VolumePath(VolumePath::new(volume.clone(), "/Documents/readme.txt").unwrap());
        let sibling = Resource::VolumePath(
            VolumePath::new(volume.clone(), "/Documents-old/readme.txt").unwrap(),
        );
        let other_volume_child =
            Resource::VolumePath(VolumePath::new(other_volume, "/Documents/readme.txt").unwrap());
        let mut policy = StaticPolicyAuthority::new();
        policy.grant(principal.clone(), scope, Permission::Read);

        for resource in [child] {
            assert_eq!(
                policy
                    .authorize(&AuthorizationRequest {
                        principal: principal.clone(),
                        resource,
                        permission: Permission::Read,
                    })
                    .unwrap(),
                Decision::Allow
            );
        }
        for resource in [sibling, other_volume_child] {
            assert_eq!(
                policy
                    .authorize(&AuthorizationRequest {
                        principal: principal.clone(),
                        resource,
                        permission: Permission::Read,
                    })
                    .unwrap(),
                Decision::Deny
            );
        }
    }

    #[test]
    fn constrained_decision_is_typed() {
        let decision = Decision::Constrained {
            constraints: vec![
                Constraint::ReadOnly,
                Constraint::PathLimited("/home/alice".to_owned()),
            ],
        };
        assert!(matches!(decision, Decision::Constrained { .. }));
    }

    #[test]
    fn system_is_an_explicit_principal_not_an_implicit_bypass() {
        let request = AuthorizationRequest {
            principal: Principal::System,
            resource: Resource::System,
            permission: Permission::Manage,
        };
        assert_eq!(
            StaticPolicyAuthority::new().authorize(&request).unwrap(),
            Decision::Deny
        );
    }
}
