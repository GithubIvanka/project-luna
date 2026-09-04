//! Configuration model and layered lookup contracts for Project Luna.
//!
//! Configuration storage is separate from authorization and runtime state. The
//! actual on-disk representation may use TOML, but serialization is intentionally
//! outside this first contract.

use std::collections::BTreeMap;
use std::fmt;

use luna_common::{BundleId, UserId};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ConfigScope {
    System,
    User(UserId),
    Application { user: UserId, application: BundleId },
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ConfigKey(String);
impl ConfigKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigValue(String);
impl ConfigValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn into_string(self) -> String {
        self.0
    }
}
#[derive(Debug)]
pub enum ConfigError {
    InvalidKey,
    ReadOnly,
    Storage(String),
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey => f.write_str("invalid configuration key"),
            Self::ReadOnly => f.write_str("configuration scope is read-only"),
            Self::Storage(message) => write!(f, "configuration storage error: {message}"),
        }
    }
}
impl std::error::Error for ConfigError {}
pub trait ConfigStore {
    fn get(&self, key: &ConfigKey) -> Result<Option<ConfigValue>, ConfigError>;
    fn set(&mut self, key: ConfigKey, value: ConfigValue) -> Result<(), ConfigError>;
    fn remove(&mut self, key: &ConfigKey) -> Result<(), ConfigError>;
}
#[derive(Clone, Debug, Default)]
pub struct MemoryConfigStore {
    values: BTreeMap<ConfigKey, ConfigValue>,
}
impl MemoryConfigStore {
    pub fn new() -> Self {
        Self::default()
    }
}
impl ConfigStore for MemoryConfigStore {
    fn get(&self, key: &ConfigKey) -> Result<Option<ConfigValue>, ConfigError> {
        Ok(self.values.get(key).cloned())
    }
    fn set(&mut self, key: ConfigKey, value: ConfigValue) -> Result<(), ConfigError> {
        if key.as_str().trim().is_empty() {
            return Err(ConfigError::InvalidKey);
        }
        self.values.insert(key, value);
        Ok(())
    }
    fn remove(&mut self, key: &ConfigKey) -> Result<(), ConfigError> {
        self.values.remove(key);
        Ok(())
    }
}

/// Layered application lookup: user/application override -> application default -> system default.
#[derive(Clone, Debug, Default)]
pub struct LayeredConfig {
    system: MemoryConfigStore,
    applications: BTreeMap<BundleId, MemoryConfigStore>,
    users: BTreeMap<UserId, MemoryConfigStore>,
    user_applications: BTreeMap<(UserId, BundleId), MemoryConfigStore>,
}
impl LayeredConfig {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn store_mut(&mut self, scope: &ConfigScope) -> &mut MemoryConfigStore {
        match scope {
            ConfigScope::System => &mut self.system,
            ConfigScope::User(user) => self.users.entry(user.clone()).or_default(),
            ConfigScope::Application { user, application } => self
                .user_applications
                .entry((user.clone(), application.clone()))
                .or_default(),
        }
    }
    pub fn application_defaults_mut(&mut self, application: &BundleId) -> &mut MemoryConfigStore {
        self.applications.entry(application.clone()).or_default()
    }
    pub fn get_application(
        &self,
        user: &UserId,
        application: &BundleId,
        key: &ConfigKey,
    ) -> Result<Option<ConfigValue>, ConfigError> {
        if let Some(store) = self
            .user_applications
            .get(&(user.clone(), application.clone()))
            && let Some(value) = store.get(key)?
        {
            return Ok(Some(value));
        }
        if let Some(store) = self.applications.get(application)
            && let Some(value) = store.get(key)?
        {
            return Ok(Some(value));
        }
        self.system.get(key)
    }
}
#[cfg(test)]
mod tests {
    use super::{ConfigKey, ConfigScope, ConfigStore, ConfigValue, LayeredConfig};
    use luna_common::{BundleId, UserId};
    #[test]
    fn application_configuration_uses_user_then_app_then_system_precedence() {
        let user = UserId::from("alice");
        let application = BundleId::from("example.app");
        let key = ConfigKey::new("theme");
        let mut config = LayeredConfig::new();
        config
            .store_mut(&ConfigScope::System)
            .set(key.clone(), ConfigValue::new("system"))
            .expect("system value");
        config
            .application_defaults_mut(&application)
            .set(key.clone(), ConfigValue::new("application"))
            .expect("application value");
        assert_eq!(
            config
                .get_application(&user, &application, &key)
                .unwrap()
                .unwrap()
                .as_str(),
            "application"
        );
        config
            .store_mut(&ConfigScope::Application {
                user: user.clone(),
                application: application.clone(),
            })
            .set(key.clone(), ConfigValue::new("user"))
            .expect("user override");
        assert_eq!(
            config
                .get_application(&user, &application, &key)
                .unwrap()
                .unwrap()
                .as_str(),
            "user"
        );
    }
}
