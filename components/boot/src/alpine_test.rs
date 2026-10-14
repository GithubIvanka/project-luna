//! Experimental Alpine boot paths through the Luna UEFI loader.
//!
//! The control path keeps Alpine's stock initramfs flow. The direct-root
//! experiment removes the initramfs and lets the Luna kernel mount the Alpine
//! root filesystem directly and execute Alpine /sbin/init.

use alloc::string::String;

use uefi::boot::{self, open_protocol_exclusive};
use uefi::runtime::{self, ResetType};

use crate::e820::E820Extension;
use crate::error::BootResult;
use crate::filesystem::SystemFilesystem;
use crate::handoff::{current_stack_pointer, enter_standard_linux, transition_entry_address};
use crate::kernel::KernelLoader;
use crate::paging::prepare_identity_map;
use crate::target::BootTarget;

#[cfg(feature = "alpine-luna-kernel")]
const TEST_KERNEL: &str = "/kernels/7.2.4/bzImage";
#[cfg(not(feature = "alpine-luna-kernel"))]
const TEST_KERNEL: &str = "/alpine/vmlinuz-virt";
#[cfg(not(feature = "alpine-direct-root"))]
const ALPINE_INITRAMFS: &str = "/alpine/initramfs-virt";

#[cfg(feature = "alpine-luna-kernel")]
const TEST_KERNEL_ID: &str = "7.2.4";
#[cfg(not(feature = "alpine-luna-kernel"))]
const TEST_KERNEL_ID: &str = "6.18.52-0-virt";

pub fn boot_flow() -> BootResult<()> {
    debug_stage("Luna: Alpine test path start\r\n");
    let mut filesystem = SystemFilesystem::open()?;
    debug_stage("Luna: Alpine LUNA-SYS ready\r\n");

    let system_guid = gpt_guid_string(filesystem.system_partition().partition_guid);
    let mut cmdline = String::from("console=tty0 console=ttyS0,115200n8 modloop=none");
    #[cfg(feature = "alpine-direct-root")]
    cmdline.push_str(" root=PARTUUID=");
    cmdline.push_str(&system_guid);
    cmdline.push_str(" rootfstype=ext4 rootflags=rw rootwait luna_direct_root=1 init=/sbin/init");
    #[cfg(feature = "alpine-debug")]
    cmdline.push_str(" debug_init loglevel=7");
    #[cfg(not(feature = "alpine-debug"))]
    cmdline.push_str(" quiet");

    let target = BootTarget::new(
        "Alpine Linux",
        "alpine",
        "3.24.2",
        "",
        "",
        "",
        TEST_KERNEL,
        TEST_KERNEL_ID,
    )
    .with_cmdline(cmdline);

    let mut prepared = {
        debug_stage("Luna: preparing Alpine root without initramfs\r\n");
        KernelLoader::new(&mut filesystem).prepare(&target)?
    };
    #[cfg(not(feature = "alpine-direct-root"))]
    let mut prepared = {
        let prepared =
            KernelLoader::new(&mut filesystem).prepare_with_initrd(&target, ALPINE_INITRAMFS)?;
        debug_stage("Luna: Alpine kernel + initramfs prepared\r\n");
        prepared
    };

    let framebuffer_reservation = prepared.boot_params.preserve_efi_framebuffer()?;
    if framebuffer_reservation.is_some() {
        debug_stage("Luna: EFI framebuffer handoff prepared\r\n");
    }

    let mut e820_ext = E820Extension::allocate()?;
    let (page_table, page_table_pages) =
        prepare_identity_map(transition_entry_address(), current_stack_pointer())?;
    let mut reserved = prepared.allocations.clone();
    reserved.push((e820_ext.address, e820_ext.allocation_pages));
    reserved.push((page_table, page_table_pages));
    if let Some((address, pages)) = framebuffer_reservation {
        reserved.push((address, pages));
    }
    debug_stage("Luna: Alpine transition map ready\r\n");

    let final_map = unsafe { boot::exit_boot_services(None) };
    if prepared
        .boot_params
        .set_e820_from_map_reserved(&final_map, &reserved, &mut e820_ext, 0)
        .is_err()
    {
        runtime::reset(ResetType::COLD, uefi::Status::ABORTED, None);
    }
    prepared.boot_params.set_setup_data(e820_ext.address)?;

    unsafe {
        core::ptr::copy_nonoverlapping(
            prepared.boot_params.as_bytes().as_ptr(),
            prepared.boot_params_address as *mut u8,
            prepared.boot_params.as_bytes().len(),
        );
        enter_standard_linux(
            prepared.kernel_entry,
            prepared.boot_params_address,
            page_table,
        );
    }
}

fn gpt_guid_string(raw: [u8; 16]) -> String {
    let b = raw;
    let mut out = String::new();
    use core::fmt::Write;
    let _ = write!(
        &mut out,
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[3],
        b[2],
        b[1],
        b[0],
        b[5],
        b[4],
        b[7],
        b[6],
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15]
    );
    out
}

fn debug_stage(message: &str) {
    if let Ok(serial_handle) =
        boot::get_handle_for_protocol::<uefi::proto::console::serial::Serial>()
    {
        if let Ok(mut serial) =
            open_protocol_exclusive::<uefi::proto::console::serial::Serial>(serial_handle)
        {
            let _ = serial.write(message.as_bytes());
        }
    }
}
