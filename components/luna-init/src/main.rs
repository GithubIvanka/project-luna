//! Project Luna direct PID1.
//!
//! `luna-init` is executed directly by the Linux kernel as PID 1. The kernel
//! supplies LunaBootHandoffV1 through file descriptor 3. No initramfs-style
//! bootstrap, BusyBox, root switching, or second init process is required.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::FromRawFd;
use std::time::Duration;

const HANDOFF_MAGIC: &[u8; 8] = b"LUNAHD01";
const HANDOFF_MAJOR: u16 = 1;
const HANDOFF_HEADER_SIZE: usize = 84;
const HANDOFF_ALIGNED_HEADER_SIZE: usize = 88;
const HANDOFF_MAX_SIZE: usize = 64 * 1024;
const RECORD_HEADER_SIZE: usize = 8;
const RECORD_ALIGN: usize = 8;
const RECORD_SYSTEM_PARTITION: u16 = 1;
const RECORD_DATA_PARTITION: u16 = 2;
const RECORD_SYSTEM_IMAGE: u16 = 3;
const RECORD_KERNEL_IDENTITY: u16 = 4;
const RECORD_LUNA_INIT_IMAGE: u16 = 5;
const RECORD_BOOT_MODE: u16 = 6;
const RECORD_BOOT_STATE: u16 = 7;
const CHECKSUM_OFFSET: usize = 52;
const CHECKSUM_SIZE: usize = 32;

fn main() -> ! {
    match run() {
        Ok(()) => reap_forever(),
        Err(error) => panic!("luna-init: {error}"),
    }
}

fn run() -> Result<(), String> {
    let mut handoff = read_handoff_fd3()?;
    validate_handoff(&mut handoff)?;

    // The launcher uses this path only as the kernel's memory-backed staging
    // name. Remove the namespace entry once the executable has been loaded;
    // the active executable mapping remains valid after unlink.
    let _ = fs::remove_file("/luna-init");

    write_stderr("Luna: luna-init is running as PID 1\n");
    Ok(())
}

fn read_handoff_fd3() -> Result<Vec<u8>, String> {
    // FD 3 is deliberately transferred without close-on-exec by the kernel
    // launcher. Ownership is taken here so it is closed before children are
    // ever started.
    let mut file = unsafe { File::from_raw_fd(3) };
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("seek handoff fd 3: {e}"))?;

    let mut bytes = Vec::with_capacity(HANDOFF_MAX_SIZE.min(4096));
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("read handoff fd 3: {e}"))?;

    if bytes.len() < HANDOFF_HEADER_SIZE {
        return Err(format!("handoff is too small: {} bytes", bytes.len()));
    }
    if bytes.len() > HANDOFF_MAX_SIZE {
        return Err(format!("handoff is too large: {} bytes", bytes.len()));
    }
    Ok(bytes)
}

fn validate_handoff(bytes: &mut [u8]) -> Result<(), String> {
    if &bytes[0..8] != HANDOFF_MAGIC {
        return Err("handoff magic mismatch".to_owned());
    }

    let major = read_u16(bytes, 8)?;
    let header_size = read_u32(bytes, 12)? as usize;
    let total_size = read_u32(bytes, 16)? as usize;
    let records_offset = read_u64(bytes, 36)? as usize;
    let records_size = read_u64(bytes, 44)? as usize;

    if major != HANDOFF_MAJOR {
        return Err(format!("unsupported handoff ABI major: {major}"));
    }
    if header_size != HANDOFF_HEADER_SIZE {
        return Err(format!("unexpected handoff header size: {header_size}"));
    }
    if total_size != bytes.len() {
        return Err(format!(
            "handoff total_size {} does not match fd size {}",
            total_size,
            bytes.len()
        ));
    }
    if records_offset != HANDOFF_ALIGNED_HEADER_SIZE
        || records_offset % RECORD_ALIGN != 0
        || !range_ok(records_offset, records_size, total_size)
    {
        return Err("invalid handoff record area".to_owned());
    }

    validate_checksum(bytes)?;

    let records_end = records_offset + records_size;
    let mut offset = records_offset;
    let mut seen = [false; 7];
    let mut init_size = 0u64;

    while offset < records_end {
        if records_end - offset < RECORD_HEADER_SIZE {
            return Err("truncated handoff record header".to_owned());
        }

        let record_type = read_u16(bytes, offset)?;
        let record_size = read_u32(bytes, offset + 4)? as usize;
        let payload = offset
            .checked_add(RECORD_HEADER_SIZE)
            .ok_or_else(|| "handoff record offset overflow".to_owned())?;
        if !range_ok(payload, record_size, records_end) {
            return Err(format!("record {record_type} exceeds handoff"));
        }

        match record_type {
            RECORD_SYSTEM_PARTITION => seen[0] = true,
            RECORD_DATA_PARTITION => seen[1] = true,
            RECORD_SYSTEM_IMAGE => seen[2] = true,
            RECORD_KERNEL_IDENTITY => seen[3] = true,
            RECORD_LUNA_INIT_IMAGE => {
                if seen[4] || record_size != 56 {
                    return Err("invalid Luna init image record".to_owned());
                }
                let address = read_u64(bytes, payload)?;
                init_size = read_u64(bytes, payload + 8)?;
                if address == 0 || init_size == 0 {
                    return Err("invalid Luna init image range".to_owned());
                }
                seen[4] = true;
            }
            RECORD_BOOT_MODE => {
                if record_size != 1 {
                    return Err("invalid boot mode record".to_owned());
                }
                seen[5] = true;
            }
            RECORD_BOOT_STATE => {
                if record_size != 24 {
                    return Err("invalid boot state record".to_owned());
                }
                seen[6] = true;
            }
            _ => {}
        }

        let next = payload
            .checked_add(record_size)
            .and_then(|value| value.checked_add(RECORD_ALIGN - 1))
            .ok_or_else(|| "handoff record alignment overflow".to_owned())?
            & !(RECORD_ALIGN - 1);
        if next <= offset || next > records_end {
            return Err("invalid handoff record alignment".to_owned());
        }
        offset = next;
    }

    if seen.iter().any(|present| !present) {
        return Err("handoff is missing a required record".to_owned());
    }
    if init_size == 0 {
        return Err("handoff has no luna-init image".to_owned());
    }

    Ok(())
}

fn validate_checksum(bytes: &mut [u8]) -> Result<(), String> {
    if CHECKSUM_OFFSET + CHECKSUM_SIZE > bytes.len() {
        return Err("handoff checksum field is out of range".to_owned());
    }

    let expected = bytes[CHECKSUM_OFFSET..CHECKSUM_OFFSET + CHECKSUM_SIZE].to_vec();
    bytes[CHECKSUM_OFFSET..CHECKSUM_OFFSET + CHECKSUM_SIZE].fill(0);
    let digest = blake3::hash(bytes);
    bytes[CHECKSUM_OFFSET..CHECKSUM_OFFSET + CHECKSUM_SIZE]
        .copy_from_slice(&expected);

    if digest.as_bytes() != expected.as_slice() {
        return Err("handoff BLAKE3 checksum mismatch".to_owned());
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| "u16 read overflow".to_owned())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "u16 read outside handoff".to_owned())?;
    Ok(u16::from_le_bytes(
        slice.try_into().map_err(|_| "invalid u16 slice".to_owned())?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| "u32 read overflow".to_owned())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "u32 read outside handoff".to_owned())?;
    Ok(u32::from_le_bytes(
        slice.try_into().map_err(|_| "invalid u32 slice".to_owned())?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| "u64 read overflow".to_owned())?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| "u64 read outside handoff".to_owned())?;
    Ok(u64::from_le_bytes(
        slice.try_into().map_err(|_| "invalid u64 slice".to_owned())?,
    ))
}

fn range_ok(offset: usize, size: usize, total: usize) -> bool {
    offset <= total && size <= total - offset
}

fn write_stderr(message: &str) {
    use std::io::Write;
    let mut stderr = std::io::stderr();
    let _ = stderr.write_all(message.as_bytes());
    let _ = stderr.flush();
}

fn reap_forever() -> ! {
    loop {
        let mut status = 0;
        let pid = unsafe { libc::waitpid(-1, &mut status, 0) };
        if pid < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error();
            if errno != Some(libc::ECHILD) && errno != Some(libc::EINTR) {
                write_stderr(&format!("Luna: waitpid failed: errno {:?}\n", errno));
            }
            if errno == Some(libc::ECHILD) {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}
