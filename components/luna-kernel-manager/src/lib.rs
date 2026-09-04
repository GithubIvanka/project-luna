//! Kernel model and compatibility query boundary for Project Luna.
//!
//! The kernel manager owns kernel inventory and query state. Installation,
//! update and removal are executed by update-manager.

use luna_common::Version;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct KernelRef {
    version: Version,
}

impl KernelRef {
    pub const fn new(version: Version) -> Self {
        Self { version }
    }
    pub const fn version(&self) -> Version {
        self.version
    }
}

/// Current, previous and factory kernel identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelSelection {
    current: KernelRef,
    previous: Option<KernelRef>,
    factory: KernelRef,
}

impl KernelSelection {
    pub const fn new(current: KernelRef, previous: Option<KernelRef>, factory: KernelRef) -> Self {
        Self {
            current,
            previous,
            factory,
        }
    }

    pub const fn current(&self) -> &KernelRef {
        &self.current
    }
    pub const fn previous(&self) -> Option<&KernelRef> {
        self.previous.as_ref()
    }
    pub const fn factory(&self) -> &KernelRef {
        &self.factory
    }
}

pub trait KernelQuery {
    type Error;
    fn selection(&self) -> Result<KernelSelection, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{KernelRef, KernelSelection};
    use luna_common::Version;

    #[test]
    fn selection_keeps_current_previous_and_factory_distinct() {
        let current = KernelRef::new(Version::new(9, 0, 0));
        let previous = KernelRef::new(Version::new(8, 2, 0));
        let factory = KernelRef::new(Version::new(7, 0, 0));
        let selection =
            KernelSelection::new(current.clone(), Some(previous.clone()), factory.clone());
        assert_eq!(selection.current(), &current);
        assert_eq!(selection.previous(), Some(&previous));
        assert_eq!(selection.factory(), &factory);
    }
}
