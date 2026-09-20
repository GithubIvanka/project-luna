//! Linux entry transition and Luna Handoff ABI v1 serialization.

use alloc::vec::Vec;
use core::arch::{asm, global_asm};

use uefi::boot::{self, AllocateType, MemoryType, PAGE_SIZE};

use crate::boot_params::BootParams;
use crate::error::{BootError, BootResult};
use crate::gpt::Partition;
use crate::target::BootTarget;

const ABI_MAJOR: u16 = 1;
const ABI_MINOR: u16 = 0;
const HEADER_SIZE: usize = 84;
const HEADER_ALIGNED_SIZE: usize = 88;
const SETUP_DATA_NODE_SIZE: usize = 16;
const RECORD_ALIGN: usize = 8;
const SETUP_DATA_TYPE: u32 = 0x4c55_4e41;
const SETUP_PROGRESS_TYPE: u32 = 0x4c55_4e50; // "LUNP"
const MAX_HANDOFF_SIZE: usize = 64 * 1024;
const PROGRESS_PAYLOAD_SIZE: usize = 32;

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum BootProgressStage {
    KernelHandoff = 3,
    KernelStarted = 4,
    InitStarted = 5,
    InitReady = 6,
    SystemRuntimeStarted = 7,
    Success = 8,
    Failed = 255,
}

pub const RECORD_SYSTEM_PARTITION: u16 = 1;
pub const RECORD_DATA_PARTITION: u16 = 2;
pub const RECORD_SYSTEM_IMAGE: u16 = 3;
pub const RECORD_KERNEL_IDENTITY: u16 = 4;
pub const RECORD_LUNA_INIT_IMAGE: u16 = 5;
pub const RECORD_BOOT_MODE: u16 = 6;
pub const RECORD_BOOT_STATE: u16 = 7;
pub const RECORD_RECOVERY_DATA_IMAGE: u16 = 8;

const PARTITION_FLAG_ABSENT: u16 = 1;

#[derive(Clone, Copy)]
pub enum BootMode {
    Normal = 0,
    Detailed = 1,
    Recovery = 2,
    Factory = 3,
    #[allow(dead_code)]
    External = 4,
}

#[derive(Clone, Copy, Default)]
pub struct BootState {
    pub fallback_depth: u8,
    pub previous_attempt_failed: bool,
    pub previous_attempt_id: u64,
    pub failure_code: u32,
}

pub struct LunaHandoff {
    /// Physical address of the Linux `struct setup_data` node.
    pub address: u64,
    /// Size of the Luna ABI payload, excluding the Linux setup_data node header.
    pub size: usize,
    /// Full number of pages reserved for the node plus payload.
    pub allocation_pages: usize,
}

impl LunaHandoff {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        target: &BootTarget,
        mode: BootMode,
        state: BootState,
        attempt_id: u64,
        system: &Partition,
        data: Option<&Partition>,
        manifest_bytes: &[u8],
        image_digest: &[u8; 32],
        kernel: &PreparedIdentity,
        init_address: u64,
        init_size: usize,
        init_digest: [u8; 32],
    ) -> BootResult<Self> {
        let manifest_identity = *blake3::hash(manifest_bytes).as_bytes();

        let mut bytes = Vec::with_capacity(1024);
        bytes.resize(HEADER_ALIGNED_SIZE, 0);
        push_partition_record(&mut bytes, RECORD_SYSTEM_PARTITION, Some(system))?;
        push_partition_record(&mut bytes, RECORD_DATA_PARTITION, data)?;
        push_system_image_record(&mut bytes, target, &manifest_identity, image_digest)?;
        push_kernel_record(&mut bytes, target, kernel)?;
        let mut init = Vec::with_capacity(56);
        init.extend_from_slice(&init_address.to_le_bytes());
        init.extend_from_slice(&(init_size as u64).to_le_bytes());
        init.extend_from_slice(&init_digest);
        init.extend_from_slice(&0u32.to_le_bytes());
        init.extend_from_slice(&0u32.to_le_bytes());
        push_record(&mut bytes, RECORD_LUNA_INIT_IMAGE, 0, &init)?;
        push_record(&mut bytes, RECORD_BOOT_MODE, 0, &[(mode as u8)])?;
        let mut boot_state = Vec::with_capacity(24);
        boot_state.push(1);
        boot_state.push(state.fallback_depth);
        boot_state.push(state.previous_attempt_failed as u8);
        boot_state.push(0);
        boot_state.extend_from_slice(&state.previous_attempt_id.to_le_bytes());
        boot_state.extend_from_slice(&state.failure_code.to_le_bytes());
        boot_state.extend_from_slice(&0u32.to_le_bytes());
        boot_state.extend_from_slice(&0u32.to_le_bytes());
        push_record(&mut bytes, RECORD_BOOT_STATE, 0, &boot_state)?;
        if target.is_recovery {
            let recovery_digest = target
                .recovery_data_digest
                .ok_or(BootError::RecoveryUnavailable)?;
            push_recovery_data_record(&mut bytes, target, &recovery_digest)?;
        }

        if bytes.len() > MAX_HANDOFF_SIZE || bytes.len() > u32::MAX as usize {
            return Err(BootError::Unsupported("Luna boot handoff is too large"));
        }
        let payload_offset = HEADER_ALIGNED_SIZE;
        let total_size = bytes.len();
        bytes[0..8].copy_from_slice(&magic().to_le_bytes());
        bytes[8..10].copy_from_slice(&ABI_MAJOR.to_le_bytes());
        bytes[10..12].copy_from_slice(&ABI_MINOR.to_le_bytes());
        bytes[12..16].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&(total_size as u32).to_le_bytes());
        bytes[20..24].copy_from_slice(&0u32.to_le_bytes());
        bytes[24..28].copy_from_slice(&0u32.to_le_bytes());
        bytes[28..36].copy_from_slice(&attempt_id.to_le_bytes());
        bytes[36..44].copy_from_slice(&(payload_offset as u64).to_le_bytes());
        bytes[44..52].copy_from_slice(&((total_size - payload_offset) as u64).to_le_bytes());
        bytes[52..84].fill(0);
        let mut checksum_bytes = bytes.clone();
        checksum_bytes[52..84].fill(0);
        let checksum = *blake3::hash(&checksum_bytes).as_bytes();
        bytes[52..84].copy_from_slice(&checksum);

        let total_node_size = SETUP_DATA_NODE_SIZE
            .checked_add(bytes.len())
            .ok_or(BootError::InvalidKernel)?;
        let pages = div_ceil(total_node_size, PAGE_SIZE);
        let allocation = boot::allocate_pages(
            AllocateType::MaxAddress(0xffff_ffff),
            MemoryType::LOADER_DATA,
            pages.max(1),
        )
        .map_err(|_| BootError::MemoryAllocationFailed)?;
        let address = allocation.as_ptr() as u64;
        unsafe {
            core::ptr::write_bytes(address as *mut u8, 0, pages * PAGE_SIZE);
            let node = address as *mut u8;
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                node.add(SETUP_DATA_NODE_SIZE),
                bytes.len(),
            );
            (node.add(0) as *mut u64).write(0);
            (node.add(8) as *mut u32).write(SETUP_DATA_TYPE);
            (node.add(12) as *mut u32).write(bytes.len() as u32);
        }
        Ok(Self {
            address,
            size: bytes.len(),
            allocation_pages: pages,
        })
    }

    pub fn link_next(&mut self, next: u64) {
        unsafe {
            (self.address as *mut u8 as *mut u64).write(next);
        }
    }
}

pub struct LunaBootProgress {
    pub address: u64,
    pub allocation_pages: usize,
}

impl LunaBootProgress {
    pub fn allocate(attempt_id: u64) -> BootResult<Self> {
        let pages = div_ceil(SETUP_DATA_NODE_SIZE + PROGRESS_PAYLOAD_SIZE, PAGE_SIZE).max(1);
        let allocation = boot::allocate_pages(
            AllocateType::MaxAddress(0xffff_ffff),
            MemoryType::LOADER_DATA,
            pages,
        )
        .map_err(|_| BootError::MemoryAllocationFailed)?;
        let address = allocation.as_ptr() as u64;
        unsafe {
            core::ptr::write_bytes(address as *mut u8, 0, pages * PAGE_SIZE);
            let node = address as *mut u8;
            (node.add(0) as *mut u64).write(0);
            (node.add(8) as *mut u32).write(SETUP_PROGRESS_TYPE);
            (node.add(12) as *mut u32).write(PROGRESS_PAYLOAD_SIZE as u32);
            core::ptr::copy_nonoverlapping(b"LUNAPR01".as_ptr(), node.add(SETUP_DATA_NODE_SIZE), 8);
            node.add(SETUP_DATA_NODE_SIZE + 8).write(1);
            node.add(SETUP_DATA_NODE_SIZE + 9)
                .write(BootProgressStage::KernelHandoff as u8);
            (node.add(SETUP_DATA_NODE_SIZE + 12) as *mut u64).write(attempt_id);
        }
        Ok(Self {
            address,
            allocation_pages: pages,
        })
    }
}

pub struct PreparedIdentity {
    pub kernel_digest: [u8; 32],
}

fn push_partition_record(
    bytes: &mut Vec<u8>,
    ty: u16,
    partition: Option<&Partition>,
) -> BootResult<()> {
    let (flags, disk_guid, partition_guid, label) = match partition {
        Some(partition) => (
            0,
            partition.disk_guid,
            partition.partition_guid,
            partition.label.as_bytes(),
        ),
        None if ty == RECORD_DATA_PARTITION => {
            (PARTITION_FLAG_ABSENT, [0u8; 16], [0u8; 16], &[][..])
        }
        None => return Err(BootError::InvalidFilesystem),
    };

    if label.len() > u16::MAX as usize {
        return Err(BootError::Unsupported("partition label is too long"));
    }
    let mut payload = Vec::with_capacity(36 + label.len());
    payload.extend_from_slice(&disk_guid);
    payload.extend_from_slice(&partition_guid);
    payload.extend_from_slice(&(label.len() as u16).to_le_bytes());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(label);
    push_record(bytes, ty, flags, &payload)
}

fn push_system_image_record(
    bytes: &mut Vec<u8>,
    target: &BootTarget,
    manifest: &[u8; 32],
    image: &[u8; 32],
) -> BootResult<()> {
    let family = target.image_family.as_bytes();
    let version = target.system_version.as_bytes();
    let filename = target
        .system_image_path
        .strip_prefix("/images/")
        .ok_or(BootError::InvalidConfig)?;
    let filename = filename.as_bytes();
    if family.len() > u16::MAX as usize
        || version.len() > u16::MAX as usize
        || filename.len() > u16::MAX as usize
    {
        return Err(BootError::Unsupported(
            "System Image identity string is too long",
        ));
    }
    let mut payload = Vec::with_capacity(72 + family.len() + version.len() + filename.len());
    payload.extend_from_slice(&(family.len() as u16).to_le_bytes());
    payload.extend_from_slice(&(version.len() as u16).to_le_bytes());
    payload.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(manifest);
    payload.extend_from_slice(image);
    payload.extend_from_slice(family);
    payload.extend_from_slice(version);
    payload.extend_from_slice(filename);
    push_record(bytes, RECORD_SYSTEM_IMAGE, 0, &payload)
}

fn push_recovery_data_record(
    bytes: &mut Vec<u8>,
    target: &BootTarget,
    digest: &[u8; 32],
) -> BootResult<()> {
    let path = target
        .recovery_data_path
        .as_deref()
        .ok_or(BootError::RecoveryUnavailable)?;
    let filename = path
        .strip_prefix("/recovery/")
        .ok_or(BootError::RecoveryUnavailable)?;
    let version = target.system_version.as_bytes();
    let filename = filename.as_bytes();
    if version.len() > u16::MAX as usize || filename.len() > u16::MAX as usize {
        return Err(BootError::Unsupported(
            "Recovery DATA Image identity string is too long",
        ));
    }
    let mut payload = Vec::with_capacity(40 + version.len() + filename.len());
    payload.extend_from_slice(&(version.len() as u16).to_le_bytes());
    payload.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(digest);
    payload.extend_from_slice(version);
    payload.extend_from_slice(filename);
    push_record(bytes, RECORD_RECOVERY_DATA_IMAGE, 0, &payload)
}

fn push_kernel_record(
    bytes: &mut Vec<u8>,
    target: &BootTarget,
    kernel: &PreparedIdentity,
) -> BootResult<()> {
    let release = target.kernel_id.as_bytes();
    let artifact = target.kernel_path.as_bytes();
    let format = b"bzImage";
    if release.len() > u16::MAX as usize || artifact.len() > u16::MAX as usize {
        return Err(BootError::Unsupported("kernel identity string is too long"));
    }
    let mut payload = Vec::with_capacity(72 + release.len() + artifact.len() + format.len());
    payload.extend_from_slice(&(release.len() as u16).to_le_bytes());
    payload.extend_from_slice(&(artifact.len() as u16).to_le_bytes());
    payload.extend_from_slice(&(format.len() as u16).to_le_bytes());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&kernel.kernel_digest);
    payload.extend_from_slice(release);
    payload.extend_from_slice(artifact);
    payload.extend_from_slice(format);
    push_record(bytes, RECORD_KERNEL_IDENTITY, 0, &payload)
}

fn push_record(bytes: &mut Vec<u8>, ty: u16, flags: u16, payload: &[u8]) -> BootResult<()> {
    if payload.len() > u32::MAX as usize {
        return Err(BootError::Unsupported("handoff record is too large"));
    }
    bytes.extend_from_slice(&ty.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(payload);
    while !bytes.len().is_multiple_of(RECORD_ALIGN) {
        bytes.push(0);
    }
    Ok(())
}

fn magic() -> u64 {
    u64::from_le_bytes(*b"LUNAHD01")
}
fn div_ceil(value: usize, divisor: usize) -> usize {
    value.div_ceil(divisor)
}

/// Return the physical address of the assembly transition stub that remains
/// executing immediately after CR3 is switched.
pub fn transition_entry_address() -> u64 {
    luna_linux_entry as *const () as usize as u64
}

/// Read the stack pointer that the final transition will continue using.
pub fn current_stack_pointer() -> u64 {
    let stack: u64;
    unsafe {
        asm!("mov {}, rsp", out(reg) stack, options(nomem, nostack, preserves_flags));
    }
    stack
}

global_asm!(
    r#"
    .section .text.luna_handoff,"ax"
    .global luna_linux_entry
luna_linux_entry:
    cli
    mov cr3, rdx
    lgdt [rip + luna_boot_gdt_ptr]

    // Linux 64-bit boot protocol selectors are __BOOT_CS=0x10 and
    // __BOOT_DS=0x18. Both descriptors are flat; CS is executable/readable,
    // DS/ES/SS are writable data. FS/GS are cleared as recommended.
    mov ax, 0x18
    mov ds, ax
    mov es, ax
    mov ss, ax
    xor eax, eax
    mov fs, ax
    mov gs, ax

    mov rax, rdi
    push 0x10
    push rax
    retfq

    .align 8
luna_boot_gdt:
    .quad 0x0000000000000000
    .quad 0x0000000000000000
    .quad 0x00af9a000000ffff
    .quad 0x00cf92000000ffff
luna_boot_gdt_ptr:
    .word 0x1f
    .quad luna_boot_gdt
"#
);

unsafe extern "sysv64" {
    fn luna_linux_entry(kernel_entry: u64, boot_params: u64, page_table: u64) -> !;
}

pub struct KernelHandoff {
    pub kernel_load_address: u64,
    pub kernel_entry: u64,
    pub init_address: u64,
    pub init_size: usize,
    pub boot_params_address: u64,
    pub boot_params: BootParams,
    pub page_table: u64,
    pub luna_handoff_address: u64,
    pub luna_handoff_size: usize,
}

impl KernelHandoff {
    pub fn is_ready(&self) -> bool {
        self.kernel_load_address != 0
            && self.kernel_entry != 0
            && self.init_address != 0
            && self.init_size != 0
            && self.boot_params_address != 0
            && self.page_table != 0
            && self.luna_handoff_address != 0
            && self.luna_handoff_size != 0
    }

    pub unsafe fn enter(self) -> ! {
        unsafe { luna_linux_entry(self.kernel_entry, self.boot_params_address, self.page_table) }
    }
}
