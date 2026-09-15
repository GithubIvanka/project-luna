#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ResourceAccess {
    Read,
    Write,
    Execute,
}
