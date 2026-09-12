//! Linux bzImage loader and physical-memory preparation.

use alloc::vec;
use alloc::vec::Vec;
use core::ptr;

use uefi::boot::{self, AllocateType, MemoryType, PAGE_SIZE};

use crate::boot_params::{BootParams, LOW_MEMORY_CMDLINE_MAX};
use crate::e820::E820Entry;
use crate::error::{BootError, BootResult};
use crate::filesystem::SystemFilesystem;
use crate::linux::LinuxSetupHeader;
use crate::target::BootTarget;

pub struct PreparedKernel {
    #[allow(dead_code)]
    pub setup: LinuxSetupHeader,
    pub kernel_address: u64,
    pub kernel_entry: u64,
    pub kernel_digest: [u8; 32],
    pub init_address: u64,
    pub init_size: usize,
    pub init_digest: [u8; 32],
    pub boot_params_address: u64,
    #[allow(dead_code)]
    pub command_line_address: u64,
    pub boot_params: BootParams,
    pub allocations: Vec<(u64, usize)>,
}

pub struct KernelLoader<'a> {
    filesystem: &'a mut SystemFilesystem,
}

impl<'a> KernelLoader<'a> {
    pub fn new(filesystem: &'a mut SystemFilesystem) -> Self { Self { filesystem } }

    pub fn prepare(&mut self, target: &BootTarget) -> BootResult<PreparedKernel> {
        let kernel = self.filesystem.read_file(&target.kernel_path)?;
        let kernel_digest = *blake3::hash(&kernel).as_bytes();
        let setup = LinuxSetupHeader::parse(&kernel)?;
        if setup.xloadflags & 1 == 0 {
            return Err(BootError::Unsupported("kernel does not advertise XLF_KERNEL_64"));
        }

        // For a modern bzImage the protected-mode portion begins at
        // (setup_sects + 1) * 512. It remains compressed; Linux's own
        // decompressor is the code reached at loaded_address + 0x200.
        let protected = kernel
            .get(setup.protected_mode_offset()..)
            .ok_or(BootError::InvalidKernel)?;
        let kernel_size = setup.init_size as usize;
        if protected.len() > kernel_size {
            return Err(BootError::InvalidKernel);
        }

        let kernel_address = allocate_kernel(setup.pref_address, kernel_size, setup.kernel_alignment as u64)?;
        unsafe {
            ptr::write_bytes(kernel_address as *mut u8, 0, kernel_size);
            ptr::copy_nonoverlapping(protected.as_ptr(), kernel_address as *mut u8, protected.len());
        }

        let init = self.filesystem.read_file(&target.init_path)?;
        validate_luna_init(&init)?;
        let init_digest = *blake3::hash(&init).as_bytes();
        let init_size = init.len();
        let init_pages = div_ceil(init_size, PAGE_SIZE);
        let init_address = allocate_pages(init_pages, 0xffff_ffff)?;
        unsafe {
            ptr::write_bytes(init_address as *mut u8, 0, init_pages * PAGE_SIZE);
            ptr::copy_nonoverlapping(init.as_ptr(), init_address as *mut u8, init_size);
        }

        let mut boot_params = BootParams::zeroed();
        boot_params.copy_setup_header(&kernel)?;
        boot_params.set_loader_type(0xff);
        boot_params.set_loadflags(setup.loadflags | 0x01);
        boot_params.enable_setup_heap();

        let bp_addr = allocate_pages(1, 0xffff_ffff)?;
        unsafe { ptr::write_bytes(bp_addr as *mut u8, 0, PAGE_SIZE); }

        let cmdline = target.kernel_cmdline.as_bytes();
        let max_cmdline = setup.cmdline_size as usize;
        if cmdline.len() + 1 > max_cmdline {
            return Err(BootError::Unsupported("kernel command line exceeds Linux cmdline_size"));
        }
        let cmdline_addr = allocate_low_cmdline_page()?;
        unsafe {
            ptr::write_bytes(cmdline_addr as *mut u8, 0, PAGE_SIZE);
            ptr::copy_nonoverlapping(cmdline.as_ptr(), cmdline_addr as *mut u8, cmdline.len());
        }
        boot_params.set_cmdline(cmdline_addr)?;

        let e820 = Vec::<E820Entry>::new();
        boot_params.set_e820(&e820)?;
        unsafe {
            ptr::copy_nonoverlapping(
                boot_params.as_bytes().as_ptr(),
                bp_addr as *mut u8,
                boot_params.as_bytes().len(),
            );
        }

        let allocations = vec![
            (kernel_address, div_ceil(kernel_size, PAGE_SIZE)),
            (init_address, init_pages),
            (bp_addr, 1),
            (cmdline_addr, 1),
        ];

        Ok(PreparedKernel {
            setup,
            kernel_address,
            kernel_entry: kernel_address + setup.entry_offset() as u64,
            kernel_digest,
            init_address,
            init_size,
            init_digest,
            boot_params_address: bp_addr,
            command_line_address: cmdline_addr,
            boot_params,
            allocations,
        })
    }
}

const PT_LOAD: u32 = 1;
const PT_INTERP: u32 = 3;
const PF_W: u32 = 0x2;
const PF_X: u32 = 0x1;
const ET_EXEC: u16 = 2;
const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 0x3e;
const ELF64_HEADER_SIZE: usize = 64;
const ELF64_PROGRAM_HEADER_SIZE: usize = 56;
const MAX_LUNA_INIT_MEMORY: u64 = 512 * 1024 * 1024;
const PAGE_ALIGN: u64 = 0x1000;

fn validate_luna_init(bytes: &[u8]) -> BootResult<()> {
    if bytes.len() < ELF64_HEADER_SIZE || &bytes[0..4] != b"\x7fELF" {
        return Err(BootError::InvalidKernel);
    }
    if bytes[4] != 2 || bytes[5] != 1 || bytes[6] != 1 {
        return Err(BootError::Unsupported("luna-init must be ELF64 little-endian"));
    }

    let elf_type = read_u16(bytes, 16)?;
    if elf_type != ET_EXEC && elf_type != ET_DYN {
        return Err(BootError::Unsupported("luna-init has unsupported ELF type"));
    }
    if read_u16(bytes, 18)? != EM_X86_64 {
        return Err(BootError::Unsupported("luna-init must target x86_64"));
    }

    let entry = read_u64(bytes, 24)?;
    let phoff = read_u64(bytes, 32)?;
    let phentsize = read_u16(bytes, 54)? as u64;
    let phnum = read_u16(bytes, 56)? as u64;
    if phentsize != ELF64_PROGRAM_HEADER_SIZE as u64 || phnum == 0 {
        return Err(BootError::InvalidKernel);
    }
    let ph_table_size = phentsize.checked_mul(phnum).ok_or(BootError::InvalidKernel)?;
    let ph_end = phoff.checked_add(ph_table_size).ok_or(BootError::InvalidKernel)?;
    if phoff > bytes.len() as u64 || ph_end > bytes.len() as u64 {
        return Err(BootError::InvalidKernel);
    }

    let mut loadable_count = 0u64;
    let mut load_low = u64::MAX;
    let mut load_high = 0u64;
    let mut entry_executable = false;

    for index in 0..phnum {
        let base = phoff
            .checked_add(index.checked_mul(phentsize).ok_or(BootError::InvalidKernel)?)
            .ok_or(BootError::InvalidKernel)? as usize;
        let typ = read_u32(bytes, base)?;
        let flags = read_u32(bytes, base + 4)?;
        let p_offset = read_u64(bytes, base + 8)?;
        let vaddr = read_u64(bytes, base + 16)?;
        let paddr = read_u64(bytes, base + 24)?;
        let filesz = read_u64(bytes, base + 32)?;
        let memsz = read_u64(bytes, base + 40)?;
        let align = read_u64(bytes, base + 48)?;

        if typ == PT_INTERP {
            return Err(BootError::Unsupported("luna-init must not require a dynamic linker"));
        }
        if typ != PT_LOAD {
            continue;
        }
        loadable_count += 1;

        let file_end = p_offset.checked_add(filesz).ok_or(BootError::InvalidKernel)?;
        if file_end > bytes.len() as u64 || filesz > memsz {
            return Err(BootError::InvalidKernel);
        }
        let mem_end = vaddr.checked_add(memsz).ok_or(BootError::InvalidKernel)?;
        if paddr.checked_add(memsz).is_none() {
            return Err(BootError::InvalidKernel);
        }
        if align != 0
            && (!align.is_power_of_two() || (p_offset % align) != (vaddr % align))
        {
            return Err(BootError::Unsupported("luna-init segment alignment is invalid"));
        }
        if align > PAGE_ALIGN && (align & (PAGE_ALIGN - 1)) != 0 {
            return Err(BootError::Unsupported("luna-init segment alignment is unsupported"));
        }
        if (flags & PF_W != 0) && (flags & PF_X != 0) {
            return Err(BootError::Unsupported("luna-init violates W^X"));
        }
        if vaddr < load_low { load_low = vaddr; }
        if mem_end > load_high { load_high = mem_end; }
        if (flags & PF_X != 0) && entry >= vaddr && entry < mem_end {
            entry_executable = true;
        }
    }

    if loadable_count == 0 || !entry_executable || load_low == u64::MAX {
        return Err(BootError::InvalidKernel);
    }
    let load_span = load_high
        .checked_sub(load_low)
        .ok_or(BootError::InvalidKernel)?;
    if load_span == 0 || load_span > MAX_LUNA_INIT_MEMORY {
        return Err(BootError::Unsupported("luna-init load span exceeds safety limit"));
    }
    if load_low % PAGE_ALIGN != 0 {
        return Err(BootError::Unsupported("luna-init load base is not page aligned"));
    }

    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> BootResult<u16> {
    let value = bytes.get(offset..offset + 2).ok_or(BootError::InvalidKernel)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> BootResult<u32> {
    let value = bytes.get(offset..offset + 4).ok_or(BootError::InvalidKernel)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> BootResult<u64> {
    let value = bytes.get(offset..offset + 8).ok_or(BootError::InvalidKernel)?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

fn allocate_kernel(preferred: u64, size: usize, alignment: u64) -> BootResult<u64> {
    let pages = div_ceil(size + alignment as usize, PAGE_SIZE);
    if preferred != 0 {
        let aligned = (preferred + alignment - 1) & !(alignment - 1);
        if aligned < 0x1_0000_0000
            && aligned + size as u64 <= 0x1_0000_0000
            && let Ok(ptr) = boot::allocate_pages(
                AllocateType::Address(aligned),
                MemoryType::LOADER_DATA,
                pages,
            )
        {
            return Ok(ptr.as_ptr() as u64 + (aligned - ptr.as_ptr() as u64));
        }
    }
    let ptr = boot::allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, pages)
        .map_err(|_| BootError::MemoryAllocationFailed)?;
    let raw = ptr.as_ptr() as u64;
    Ok((raw + alignment - 1) & !(alignment - 1))
}

fn allocate_pages(pages: usize, max_address: u64) -> BootResult<u64> {
    let ptr = boot::allocate_pages(
        AllocateType::MaxAddress(max_address),
        MemoryType::LOADER_DATA,
        pages.max(1),
    ).map_err(|_| BootError::MemoryAllocationFailed)?;
    Ok(ptr.as_ptr() as u64)
}

fn allocate_low_cmdline_page() -> BootResult<u64> {
    let ptr = boot::allocate_pages(
        AllocateType::MaxAddress(LOW_MEMORY_CMDLINE_MAX),
        MemoryType::LOADER_DATA,
        1,
    ).map_err(|_| BootError::MemoryAllocationFailed)?;
    let address = ptr.as_ptr() as u64;
    if address + PAGE_SIZE as u64 > 0xA0000 {
        return Err(BootError::MemoryAllocationFailed);
    }
    Ok(address)
}

fn div_ceil(value: usize, divisor: usize) -> usize { value.div_ceil(divisor) }
