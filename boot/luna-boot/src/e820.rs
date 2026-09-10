//! Linux E820 data types used by luna-boot.

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct E820Entry {
    pub addr: u64,
    pub size: u64,
    pub typ: u32,
    pub reserved: u32,
}
