//! Central security-policy authority for Project Luna.
//!
//! This crate evaluates policy. Low-level kernel/filesystem primitives remain
//! responsible for enforcing the result.

use std::collections::BTreeSet;
use std::fmt;
use luna_common::{BundleId, RuntimeKind, UserId};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Principal { User(UserId), Application(BundleId), System }

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Resource {
    UserData(UserId),
    ApplicationData { user: UserId, application: BundleId },
    Application(BundleId),
    Runtime(RuntimeKind),
    Volume(String),
    Device(String),
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Permission { Visibility, Read, Write, Execute, Use, Manage }

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
pub struct AuthorizationRequest { pub principal: Principal, pub resource: Resource, pub permission: Permission }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityError(String);
impl SecurityError { pub fn new(message: impl Into<String>) -> Self { Self(message.into()) } }
impl fmt::Display for SecurityError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for SecurityError {}

pub trait PolicyAuthority { fn authorize(&self, request: &AuthorizationRequest) -> Result<Decision, SecurityError>; }

#[derive(Clone, Debug, Default)]
pub struct StaticPolicyAuthority { grants: BTreeSet<(Principal, Resource, Permission)> }
impl StaticPolicyAuthority {
    pub fn new() -> Self { Self::default() }
    pub fn grant(&mut self, principal: Principal, resource: Resource, permission: Permission) { self.grants.insert((principal, resource, permission)); }
    pub fn revoke(&mut self, principal: &Principal, resource: &Resource, permission: Permission) { self.grants.remove(&(principal.clone(), resource.clone(), permission)); }
    pub fn is_granted(&self, request: &AuthorizationRequest) -> bool { self.grants.contains(&(request.principal.clone(), request.resource.clone(), request.permission)) }
}
impl PolicyAuthority for StaticPolicyAuthority {
    fn authorize(&self, request: &AuthorizationRequest) -> Result<Decision, SecurityError> { Ok(if self.is_granted(request) { Decision::Allow } else { Decision::Deny }) }
}

#[cfg(test)]
mod tests {
    use super::{AuthorizationRequest, Constraint, Decision, Permission, PolicyAuthority, Principal, Resource, StaticPolicyAuthority};
    use luna_common::{BundleId, RuntimeKind, UserId};
    #[test]
    fn explicit_grant_allows_and_revoke_denies() {
        let principal=Principal::Application(BundleId::from("example.app")); let resource=Resource::UserData(UserId::from("alice"));
        let request=AuthorizationRequest{principal:principal.clone(),resource:resource.clone(),permission:Permission::Read}; let mut policy=StaticPolicyAuthority::new();
        assert_eq!(policy.authorize(&request).unwrap(),Decision::Deny); policy.grant(principal.clone(),resource.clone(),Permission::Read); assert_eq!(policy.authorize(&request).unwrap(),Decision::Allow);
        policy.revoke(&principal,&resource,Permission::Read); assert_eq!(policy.authorize(&request).unwrap(),Decision::Deny);
    }
    #[test]
    fn visibility_is_independent_from_read() {
        let principal=Principal::Application(BundleId::from("example.app")); let resource=Resource::Application(BundleId::from("example.app")); let mut policy=StaticPolicyAuthority::new();
        let visible=AuthorizationRequest{principal:principal.clone(),resource:resource.clone(),permission:Permission::Visibility};
        let readable=AuthorizationRequest{principal,resource,permission:Permission::Read};
        policy.grant(visible.principal.clone(),visible.resource.clone(),Permission::Visibility);
        assert_eq!(policy.authorize(&visible).unwrap(),Decision::Allow); assert_eq!(policy.authorize(&readable).unwrap(),Decision::Deny);
    }
    #[test]
    fn grants_are_scoped_to_principal_resource_and_permission() {
        let principal=Principal::Application(BundleId::from("example.app")); let other_principal=Principal::Application(BundleId::from("other.app")); let resource=Resource::Application(BundleId::from("example.app")); let other_resource=Resource::Application(BundleId::from("other.app"));
        let read=AuthorizationRequest{principal:principal.clone(),resource:resource.clone(),permission:Permission::Read}; let write=AuthorizationRequest{principal:principal.clone(),resource:resource.clone(),permission:Permission::Write};
        let other_principal_request=AuthorizationRequest{principal:other_principal,resource:resource.clone(),permission:Permission::Read}; let other_resource_request=AuthorizationRequest{principal,resource:other_resource,permission:Permission::Read};
        let mut policy=StaticPolicyAuthority::new(); policy.grant(read.principal.clone(),read.resource.clone(),Permission::Read);
        assert_eq!(policy.authorize(&read).unwrap(),Decision::Allow); assert_eq!(policy.authorize(&write).unwrap(),Decision::Deny); assert_eq!(policy.authorize(&other_principal_request).unwrap(),Decision::Deny); assert_eq!(policy.authorize(&other_resource_request).unwrap(),Decision::Deny);
    }
    #[test]
    fn runtime_requires_explicit_use_permission() {
        let principal=Principal::Application(BundleId::from("example.app"));
        let resource=Resource::Runtime(RuntimeKind::Glibc);
        let request=AuthorizationRequest{principal:principal.clone(),resource:resource.clone(),permission:Permission::Use};
        let mut policy=StaticPolicyAuthority::new();
        assert_eq!(policy.authorize(&request).unwrap(),Decision::Deny);
        policy.grant(principal,resource,Permission::Use);
        assert_eq!(policy.authorize(&request).unwrap(),Decision::Allow);
    }
    #[test]
    fn constrained_decision_is_typed() {
        let decision=Decision::Constrained{constraints:vec![Constraint::ReadOnly,Constraint::PathLimited("/home/alice".to_owned())]};
        assert!(matches!(decision,Decision::Constrained{..}));
    }
    #[test]
    fn system_is_an_explicit_principal_not_an_implicit_bypass() {
        let request=AuthorizationRequest{principal:Principal::System,resource:Resource::System,permission:Permission::Manage};
        assert_eq!(StaticPolicyAuthority::new().authorize(&request).unwrap(),Decision::Deny);
    }
}
