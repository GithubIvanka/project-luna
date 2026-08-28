//! Central security-policy authority for Project Luna.
//!
//! This crate evaluates policy. Low-level kernel/filesystem primitives remain
//! responsible for enforcing the result.

use std::collections::BTreeSet;
use std::fmt;
use luna_common::{BundleId, UserId};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Principal { User(UserId), Application(BundleId), System }

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Resource {
    UserData(UserId),
    ApplicationData { user: UserId, application: BundleId },
    Application(BundleId),
    Volume(String),
    Device(String),
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Permission { Read, Write, Execute, Use, Manage }

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision { Allow, Deny, Ask, Constrained { scope: String } }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationRequest { pub principal: Principal, pub resource: Resource, pub permission: Permission }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityError(String);
impl SecurityError { pub fn new(message: impl Into<String>) -> Self { Self(message.into()) } }
impl fmt::Display for SecurityError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for SecurityError {}

pub trait PolicyAuthority { fn authorize(&self, request: &AuthorizationRequest) -> Result<Decision, SecurityError>; }

/// Small deterministic policy implementation used by contract tests and early
/// prototypes. It is an explicit grant table, not the final persistent policy
/// backend or a privileged-user abstraction.
#[derive(Clone, Debug, Default)]
pub struct StaticPolicyAuthority { grants: BTreeSet<(Principal, Resource, Permission)> }
impl StaticPolicyAuthority {
    pub fn new() -> Self { Self::default() }
    pub fn grant(&mut self, principal: Principal, resource: Resource, permission: Permission) { self.grants.insert((principal, resource, permission)); }
    pub fn revoke(&mut self, principal: &Principal, resource: &Resource, permission: Permission) { self.grants.remove(&(principal.clone(), resource.clone(), permission)); }
    pub fn is_granted(&self, request: &AuthorizationRequest) -> bool { self.grants.contains(&(request.principal.clone(), request.resource.clone(), request.permission)) }
}
impl PolicyAuthority for StaticPolicyAuthority {
    fn authorize(&self, request: &AuthorizationRequest) -> Result<Decision, SecurityError> {
        Ok(if self.is_granted(request) { Decision::Allow } else { Decision::Deny })
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthorizationRequest, Decision, Permission, PolicyAuthority, Principal, Resource, StaticPolicyAuthority};
    use luna_common::{BundleId, UserId};
    #[test]
    fn explicit_grant_allows_and_revoke_denies() {
        let principal = Principal::Application(BundleId::from("example.app"));
        let resource = Resource::UserData(UserId::from("alice"));
        let request = AuthorizationRequest { principal: principal.clone(), resource: resource.clone(), permission: Permission::Read };
        let mut policy = StaticPolicyAuthority::new();
        assert_eq!(policy.authorize(&request).unwrap(), Decision::Deny);
        policy.grant(principal.clone(), resource.clone(), Permission::Read);
        assert_eq!(policy.authorize(&request).unwrap(), Decision::Allow);
        policy.revoke(&principal, &resource, Permission::Read);
        assert_eq!(policy.authorize(&request).unwrap(), Decision::Deny);
    }
    #[test]
    fn constrained_decision_remains_distinct() {
        let decision = Decision::Constrained { scope: "current-operation".to_owned() };
        assert!(matches!(decision, Decision::Constrained { .. }));
    }
}
