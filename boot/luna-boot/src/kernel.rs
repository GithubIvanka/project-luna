//! Linux bzImage loader and physical-memory preparation.

use alloc::vec;
use alloc::vec::Vec;
use core::ptr;

use uefi::boot::{self, AllocateType, MemoryType, PAGE_SIZE};

use crate::boot_params::BootParams;
use crate::e820::E820Entry;
use crate::error::{BootError, BootResult};
use crate::filesystem::SystemFilesystem;
use crate::linux::LinuxSetupHeader;
use crate::target::BootTarget;

pub struct PreparedKernel {
    pub setup: LinuxSetupHeader,
    pub kernel_address: u64,
    pub kernel_entry: u64,
    pub kernel_size: usize,
    pub kernel_digest: [u8; 32],
    pub init_address: u64,
    pub init_size: usize,
    pub init_digest: [u8; 32],
    pub boot_params_address: u64,
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
        boot_params.set_loadflags(setup.loadflags | 0x01 | 0x40);

        let bp_addr = allocate_pages(1, 0xffff_ffff)?;
        unsafe { ptr::write_bytes(bp_addr as *mut u8, 0, PAGE_SIZE); }

        let cmdline = target.kernel_cmdline.as_bytes();
        let max_cmdline = setup.cmdline_size as usize;
        if cmdline.len() + 1 > max_cmdline {
            return Err(BootError::Unsupported("kernel command line exceeds Linux cmdline_size"));
        }
        let cmdline_addr = allocate_pages(1, 0xffff_ffff)?;
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
            kernel_size: protected.len(),
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

fn validate_luna_init(bytes: &[u8]) -> BootResult<()> {
    if bytes.len() < 64 || &bytes[0..4] != b"\x7fELF" {
        return Err(BootError::InvalidKernel);
    }
    if bytes[4] != 2 || bytes[5] != 1 || bytes[6] != 1 {
        return Err(BootError::Unsupported("luna-init must be ELF64 little-endian"));
    }
    if u16::from_le_bytes([bytes[18], bytes[19]]) != 0x3e {
        return Err(BootError::Unsupported("luna-init must target x86_64"));
    }
    Ok(())
}

fn allocate_kernel(preferred: u64, size: usize, alignment: u64) -> BootResult<u64> {
    let pages = div_ceil(size + alignment as usize, PAGE_SIZE);
    if preferred != 0 {
        let aligned = (preferred + alignment - 1) & !(alignment - 1);
        if aligned < 0x1_0000_0000 && aligned + size as u64 <= 0x1_0000_0000 {
            if let Ok(ptr) = boot::allocate_pages(AllocateType::Address(aligned), MemoryType::LOADER_DATA, pages) {
                return Ok(ptr.as_ptr() as u64 + (aligned - ptr.as_ptr() as u64));
            }
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

const fn div_ceil(value: usize, divisor: usize) -> usize { (value + divisor - 1) / divisor }
