//! Central security-policy authority for Project Luna.
//!
//! This crate evaluates policy. Low-level kernel/filesystem primitives remain
//! responsible for enforcing the result.

use std::fmt;

use luna_common::{BundleId, UserId};

/// Identity that can appear in a Luna authorization request.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Principal {
    User(UserId),
    Application(BundleId),
    System,
}

/// Resource class used by the initial security contract.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Resource {
    UserData(UserId),
    ApplicationData { user: UserId, application: BundleId },
    Application(BundleId),
    Volume(String),
    Device(String),
    System,
}

/// Permission requested against a resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Permission {
    Read,
    Write,
    Execute,
    Use,
    Manage,
}

/// Result of policy evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision {
    /// Access is allowed for the requested context.
    Allow,
    /// Access is denied.
    Deny,
    /// The policy requires an explicit user/system confirmation before access.
    Ask,
    /// Access is allowed only under additional constraints defined by policy.
    Constrained { scope: String },
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

/// Central policy authority queried by runtimes and management services.
pub trait PolicyAuthority {
    fn authorize(&self, request: &AuthorizationRequest) -> Result<Decision, SecurityError>;
}

#[cfg(test)]
mod tests {
    use super::{AuthorizationRequest, Permission, PolicyAuthority, Principal, Resource};
    use luna_common::{BundleId, UserId};

    struct DenyAll;

    impl PolicyAuthority for DenyAll {
        fn authorize(
            &self,
            _request: &AuthorizationRequest,
        ) -> Result<super::Decision, super::SecurityError> {
            Ok(super::Decision::Deny)
        }
    }

    #[test]
    fn request_can_represent_application_access_to_user_data() {
        let request = AuthorizationRequest {
            principal: Principal::Application(BundleId::from("example.app")),
            resource: Resource::UserData(UserId::from("alice")),
            permission: Permission::Read,
        };

        assert!(matches!(
            DenyAll.authorize(&request),
            Ok(super::Decision::Deny)
        ));
    }

    #[test]
    fn constrained_decision_can_describe_limited_access() {
        let decision = super::Decision::Constrained {
            scope: "current-operation".to_owned(),
        };

        assert!(matches!(decision, super::Decision::Constrained { .. }));
    }
}
