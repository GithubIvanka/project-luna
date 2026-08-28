//! Kernel model and compatibility query boundary for Project Luna.

use luna_common::Version;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct KernelRef {
    version: Version,
}

impl KernelRef {
    pub const fn new(version: Version) -> Self { Self { version } }
    pub const fn version(&self) -> Version { self.version }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelSelection {
    current: KernelRef,
    previous: Option<KernelRef>,
}

impl KernelSelection {
    pub const fn new(current: KernelRef, previous: Option<KernelRef>) -> Self {
        Self { current, previous }
    }
    pub const fn current(&self) -> &KernelRef { &self.current }
    pub const fn previous(&self) -> Option<&KernelRef> { self.previous.as_ref() }
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
    fn selection_exposes_current_and_previous() {
        let current = KernelRef::new(Version::new(9, 0, 0));
        let previous = KernelRef::new(Version::new(8, 2, 0));
        let selection = KernelSelection::new(current.clone(), Some(previous.clone()));
        assert_eq!(selection.current(), &current);
        assert_eq!(selection.previous(), Some(&previous));
    }
}
