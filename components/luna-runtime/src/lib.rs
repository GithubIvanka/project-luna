//! Managed application runtime resolution for Project Luna.
//!
//! This crate resolves the typed `RuntimeKind` contract into an approved,
//! immutable runtime identity. It deliberately does not mount anything,
//! authorize anything, or launch processes. Those responsibilities stay in
//! `luna-root-mapping`, `luna-security`, and `luna-app-runtime` respectively.

use std::fmt;
use std::path::{Path, PathBuf};

use luna_common::RuntimeKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeArtifact {
    kind: RuntimeKind,
    root: PathBuf,
    loader: Option<PathBuf>,
}

impl RuntimeArtifact {
    pub fn kind(&self) -> RuntimeKind {
        self.kind
    }

    /// Physical artifact root. This is for mapping/materialization backends;
    /// callers must not expose it directly as an application-visible path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// ELF interpreter path relative to the artifact root, when the runtime
    /// has a dedicated loader.
    pub fn loader(&self) -> Option<&Path> {
        self.loader.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeResolutionError {
    Unapproved(RuntimeKind),
    MissingRoot(PathBuf),
    InvalidRoot(PathBuf),
    InvalidLoader(PathBuf),
}

impl fmt::Display for RuntimeResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unapproved(kind) => write!(f, "runtime is not approved: {kind}"),
            Self::MissingRoot(path) => write!(f, "runtime root does not exist: {}", path.display()),
            Self::InvalidRoot(path) => write!(f, "runtime root is not a directory: {}", path.display()),
            Self::InvalidLoader(path) => write!(f, "runtime loader is not a regular file: {}", path.display()),
        }
    }
}

impl std::error::Error for RuntimeResolutionError {}

#[derive(Clone, Debug)]
pub struct RuntimeResolver {
    native_root: PathBuf,
    glibc_root: Option<PathBuf>,
    bundle_root: Option<PathBuf>,
}

impl RuntimeResolver {
    pub fn new(native_root: impl Into<PathBuf>) -> Self {
        Self {
            native_root: native_root.into(),
            glibc_root: None,
            bundle_root: None,
        }
    }

    pub fn with_glibc_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.glibc_root = Some(root.into());
        self
    }

    pub fn with_bundle_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.bundle_root = Some(root.into());
        self
    }

    pub fn resolve(&self, kind: RuntimeKind) -> Result<RuntimeArtifact, RuntimeResolutionError> {
        let root = match kind {
            RuntimeKind::Luna => self.native_root.clone(),
            RuntimeKind::Glibc => self.glibc_root.clone().ok_or(RuntimeResolutionError::Unapproved(kind))?,
            RuntimeKind::Bundle => self.bundle_root.clone().ok_or(RuntimeResolutionError::Unapproved(kind))?,
        };

        if !root.exists() {
            return Err(RuntimeResolutionError::MissingRoot(root));
        }
        if !root.is_dir() {
            return Err(RuntimeResolutionError::InvalidRoot(root));
        }

        let loader = match kind {
            RuntimeKind::Luna => None,
            RuntimeKind::Glibc => {
                let candidate = root.join("lib64/ld-linux-x86-64.so.2");
                if candidate.is_file() { Some(candidate) } else { None }
            }
            RuntimeKind::Bundle => {
                let candidate = root.join("loader");
                if candidate.is_file() {
                    Some(candidate)
                } else {
                    None
                }
            }
        };

        if kind != RuntimeKind::Luna && loader.is_none() {
            return Err(RuntimeResolutionError::InvalidLoader(root.join("loader")));
        }

        Ok(RuntimeArtifact { kind, root, loader })
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeResolutionError, RuntimeResolver};
    use luna_common::RuntimeKind;
    use std::fs;

    #[test]
    fn native_runtime_resolves_without_glibc_or_bundle_registration() {
        let root = std::env::temp_dir().join(format!("luna-runtime-native-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let resolver = RuntimeResolver::new(&root);
        let artifact = resolver.resolve(RuntimeKind::Luna).unwrap();
        assert_eq!(artifact.kind(), RuntimeKind::Luna);
        assert_eq!(artifact.root(), root);
        assert!(artifact.loader().is_none());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unconfigured_glibc_is_rejected() {
        let root = std::env::temp_dir().join(format!("luna-runtime-glibc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let resolver = RuntimeResolver::new(&root);
        assert_eq!(
            resolver.resolve(RuntimeKind::Glibc),
            Err(RuntimeResolutionError::Unapproved(RuntimeKind::Glibc))
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn glibc_requires_its_loader() {
        let root = std::env::temp_dir().join(format!("luna-runtime-glibc-loader-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("lib64")).unwrap();

        let resolver = RuntimeResolver::new("/native").with_glibc_root(&root);
        assert!(matches!(
            resolver.resolve(RuntimeKind::Glibc),
            Err(RuntimeResolutionError::InvalidLoader(_))
        ));

        let _ = fs::remove_dir_all(&root);
    }
}
