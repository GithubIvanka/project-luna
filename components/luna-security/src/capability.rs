//! Typed capability identity and provider handoff primitives.
//!
//! `luna-security` decides whether an application may use a capability. This
//! module validates capability identity, resolves the provider registered for
//! that identity, and carries an already-authorized grant across the execution
//! boundary. Providers never make authorization decisions.

use crate::{Principal, SecurityError};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct CapabilityName(String);

impl CapabilityName {
    pub fn new(value: impl Into<String>) -> Result<Self, SecurityError> {
        let value = value.into();
        if value.is_empty() || value.chars().any(char::is_whitespace) {
            return Err(SecurityError::new(
                "capability name must not be empty or contain whitespace",
            ));
        }
        if value.starts_with('.') || value.ends_with('.') || value.contains("..") {
            return Err(SecurityError::new(
                "capability name has an invalid namespace form",
            ));
        }
        if !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        {
            return Err(SecurityError::new(
                "capability name contains unsupported characters",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityGrant {
    principal: Principal,
    capability: CapabilityName,
    provider: String,
}

impl CapabilityGrant {
    pub(crate) fn new(
        principal: Principal,
        capability: CapabilityName,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            principal,
            capability,
            provider: provider.into(),
        }
    }

    pub fn principal(&self) -> &Principal {
        &self.principal
    }

    pub fn capability(&self) -> &CapabilityName {
        &self.capability
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }
}

/// A provider consumes grants that have already been authorized by policy.
/// It must not expand or reinterpret the grant.
pub trait CapabilityProvider {
    fn capability(&self) -> &CapabilityName;
}

#[derive(Default)]
pub struct CapabilityRegistry {
    providers: BTreeMap<CapabilityName, String>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        capability: CapabilityName,
        provider: impl Into<String>,
    ) -> Result<(), SecurityError> {
        let provider = provider.into();
        if provider.is_empty() {
            return Err(SecurityError::new(
                "capability provider name must not be empty",
            ));
        }
        if self.providers.insert(capability, provider).is_some() {
            return Err(SecurityError::new("capability is already registered"));
        }
        Ok(())
    }

    pub fn provider_for(&self, capability: &CapabilityName) -> Option<&str> {
        self.providers.get(capability).map(String::as_str)
    }

    pub fn is_registered(&self, capability: &CapabilityName) -> bool {
        self.providers.contains_key(capability)
    }

    pub fn grant(
        &self,
        principal: Principal,
        capability: CapabilityName,
    ) -> Result<CapabilityGrant, SecurityError> {
        let provider = self
            .provider_for(&capability)
            .ok_or_else(|| SecurityError::new(format!("unknown capability: {capability}")))?;
        Ok(CapabilityGrant::new(principal, capability, provider))
    }

    pub fn with_default_providers() -> Self {
        let mut registry = Self::new();
        for (capability, provider) in [
            ("network", "network"),
            ("audio.output", "audio"),
            ("audio.input", "audio"),
            ("device.camera", "device"),
            ("device.microphone", "device"),
            ("desktop.clipboard", "desktop"),
            ("desktop.portal", "desktop"),
        ] {
            registry
                .register(
                    CapabilityName::new(capability).expect("built-in capability is valid"),
                    provider,
                )
                .expect("built-in capability is unique");
        }
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::{CapabilityName, CapabilityProvider, CapabilityRegistry};
    use crate::Principal;
    use luna_common::BundleId;

    struct TestProvider {
        capability: CapabilityName,
    }

    impl CapabilityProvider for TestProvider {
        fn capability(&self) -> &CapabilityName {
            &self.capability
        }
    }

    #[test]
    fn capability_names_are_namespaced_and_deterministic() {
        let name = CapabilityName::new("audio.output").unwrap();
        assert_eq!(name.as_str(), "audio.output");
        assert!(CapabilityName::new("audio..output").is_err());
        assert!(CapabilityName::new("audio output").is_err());
    }

    #[test]
    fn unknown_capability_cannot_be_granted() {
        let registry = CapabilityRegistry::new();
        let principal = Principal::Application(BundleId::from("example.app"));
        let result = registry.grant(principal, CapabilityName::new("network").unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn registered_capability_produces_typed_grant() {
        let mut registry = CapabilityRegistry::new();
        let name = CapabilityName::new("network").unwrap();
        registry.register(name.clone(), "network").unwrap();
        let principal = Principal::Application(BundleId::from("example.app"));
        let grant = registry.grant(principal.clone(), name.clone()).unwrap();
        assert_eq!(grant.principal(), &principal);
        assert_eq!(grant.capability(), &name);
        assert_eq!(grant.provider(), "network");
    }

    #[test]
    fn duplicate_provider_registration_is_rejected() {
        let mut registry = CapabilityRegistry::new();
        let name = CapabilityName::new("network").unwrap();
        registry.register(name.clone(), "network").unwrap();
        assert!(registry.register(name, "other").is_err());
    }

    #[test]
    fn builtin_registry_is_explicit() {
        let registry = CapabilityRegistry::with_default_providers();
        assert!(registry.is_registered(&CapabilityName::new("network").unwrap()));
        assert!(registry.is_registered(&CapabilityName::new("audio.output").unwrap()));
        assert!(!registry.is_registered(&CapabilityName::new("audio").unwrap()));
    }

    #[test]
    fn provider_identity_is_not_authorization() {
        let provider = TestProvider {
            capability: CapabilityName::new("network").unwrap(),
        };
        assert_eq!(provider.capability().as_str(), "network");
    }
}
