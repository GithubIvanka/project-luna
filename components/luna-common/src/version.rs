use std::fmt;

/// Semantic version value used by Luna's foundational APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    /// Creates a semantic version.
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub const fn major(self) -> u32 {
        self.major
    }

    pub const fn minor(self) -> u32 {
        self.minor
    }

    pub const fn patch(self) -> u32 {
        self.patch
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::Version;

    #[test]
    fn version_exposes_components_and_formats_stably() {
        let version = Version::new(1, 6, 0);

        assert_eq!(version.major(), 1);
        assert_eq!(version.minor(), 6);
        assert_eq!(version.patch(), 0);
        assert_eq!(version.to_string(), "1.6.0");
    }

    #[test]
    fn versions_have_value_semantics() {
        assert!(Version::new(1, 2, 0) < Version::new(1, 3, 0));
        assert_eq!(Version::new(2, 0, 0), Version::new(2, 0, 0));
    }
}
