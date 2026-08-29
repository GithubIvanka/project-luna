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
    pub boot_params_address: u64,
    pub command_line_address: u64,
    pub initrd_address: u64,
    pub initrd_size: usize,
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
        let setup = LinuxSetupHeader::parse(&kernel)?;
        if setup.xloadflags & 1 == 0 {
            return Err(BootError::Unsupported("kernel does not advertise XLF_KERNEL_64"));
        }

        let protected = kernel
            .get(setup.protected_mode_offset()..)
            .ok_or(BootError::InvalidKernel)?;
        let init_size = setup.init_size as usize;
        if protected.len() > init_size {
            return Err(BootError::InvalidKernel);
        }

        let kernel_address = allocate_kernel(setup.pref_address, init_size, setup.kernel_alignment as u64)?;
        unsafe {
            ptr::write_bytes(kernel_address as *mut u8, 0, init_size);
            ptr::copy_nonoverlapping(protected.as_ptr(), kernel_address as *mut u8, protected.len());
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

        let mut initrd_address = 0;
        let mut initrd_size = 0;
        let mut initrd_pages = 0;
        if !target.initrd_path.is_empty() {
            let initrd = self.filesystem.read_file(&target.initrd_path)?;
            if initrd.is_empty() {
                return Err(BootError::InvalidKernel);
            }
            initrd_size = initrd.len();
            initrd_pages = div_ceil(initrd.len(), PAGE_SIZE);
            let max = if setup.initrd_addr_max == 0 { 0xffff_ffff } else { setup.initrd_addr_max as u64 };
            initrd_address = allocate_pages(initrd_pages, max)?;
            unsafe {
                ptr::write_bytes(initrd_address as *mut u8, 0, initrd_pages * PAGE_SIZE);
                ptr::copy_nonoverlapping(initrd.as_ptr(), initrd_address as *mut u8, initrd.len());
            }
            boot_params.set_ramdisk(initrd_address, initrd.len() as u64)?;
        }

        let e820 = Vec::<E820Entry>::new();
        boot_params.set_e820(&e820)?;
        unsafe {
            ptr::copy_nonoverlapping(
                boot_params.as_bytes().as_ptr(),
                bp_addr as *mut u8,
                boot_params.as_bytes().len(),
            );
        }

        let mut allocations = vec![
            (kernel_address, div_ceil(init_size, PAGE_SIZE)),
            (bp_addr, 1),
            (cmdline_addr, 1),
        ];
        if initrd_pages != 0 {
            allocations.push((initrd_address, initrd_pages));
        }

        Ok(PreparedKernel {
            setup,
            kernel_address,
            kernel_entry: kernel_address + setup.entry_offset() as u64,
            kernel_size: protected.len(),
            boot_params_address: bp_addr,
            command_line_address: cmdline_addr,
            initrd_address,
            initrd_size,
            boot_params,
            allocations,
        })
    }
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
