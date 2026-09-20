// SPDX-License-Identifier: GPL-2.0
#![allow(unsafe_op_in_unsafe_fn)]

//! Project Luna kernel-side LunaBootHandoffV1 parser.
//!
//! This file is copied into the pinned Linux source tree by the Luna kernel
//! build tool. Parsing stays in Rust; the x86 C side only provides the Linux
//! `setup_data` head pointer and later consumes the exported state.

use core::ffi::c_void;
use core::ptr;

use kernel::bindings;

// Kbuild compiles this file as a standalone Rust crate/object. The kernel
// print macros expect this crate-root prefix to exist.
const __LOG_PREFIX: &[u8] = b"luna_boot\0";

mod blake3_verify {

    const IV: [u32; 8] = [
        0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB,
        0x5BE0CD19,
    ];
    const CHUNK_START: u32 = 1;
    const CHUNK_END: u32 = 2;
    const PARENT: u32 = 4;
    const ROOT: u32 = 8;
    const CHUNK_LEN: usize = 1024;
    const BLOCK_LEN: usize = 64;
    const SCHEDULE: [[usize; 16]; 7] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8],
        [3, 4, 10, 12, 13, 2, 7, 14, 6, 5, 9, 0, 11, 15, 8, 1],
        [10, 7, 12, 9, 14, 3, 13, 15, 4, 0, 11, 2, 5, 8, 1, 6],
        [12, 13, 9, 11, 15, 10, 14, 8, 7, 2, 5, 3, 0, 1, 6, 4],
        [9, 14, 11, 5, 8, 12, 15, 1, 13, 3, 0, 10, 2, 6, 4, 7],
        [11, 15, 5, 0, 1, 9, 8, 6, 14, 10, 2, 12, 3, 4, 7, 13],
    ];

    #[inline(always)]
    fn g(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
        s[a] = s[a].wrapping_add(s[b]).wrapping_add(x);
        s[d] = (s[d] ^ s[a]).rotate_right(16);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_right(12);
        s[a] = s[a].wrapping_add(s[b]).wrapping_add(y);
        s[d] = (s[d] ^ s[a]).rotate_right(8);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_right(7);
    }

    #[inline(always)]
    fn compress(
        cv: &[u32; 8],
        block: &[u8; 64],
        block_len: u32,
        counter: u64,
        flags: u32,
    ) -> [u32; 16] {
        let mut m = [0u32; 16];
        let mut i = 0;
        while i < 16 {
            let o = i * 4;
            m[i] = u32::from_le_bytes([block[o], block[o + 1], block[o + 2], block[o + 3]]);
            i += 1;
        }
        let mut s = [
            cv[0],
            cv[1],
            cv[2],
            cv[3],
            cv[4],
            cv[5],
            cv[6],
            cv[7],
            IV[0],
            IV[1],
            IV[2],
            IV[3],
            counter as u32,
            (counter >> 32) as u32,
            block_len,
            flags,
        ];
        let mut r = 0;
        while r < 7 {
            let p = &SCHEDULE[r];
            g(&mut s, 0, 4, 8, 12, m[p[0]], m[p[1]]);
            g(&mut s, 1, 5, 9, 13, m[p[2]], m[p[3]]);
            g(&mut s, 2, 6, 10, 14, m[p[4]], m[p[5]]);
            g(&mut s, 3, 7, 11, 15, m[p[6]], m[p[7]]);
            g(&mut s, 0, 5, 10, 15, m[p[8]], m[p[9]]);
            g(&mut s, 1, 6, 11, 12, m[p[10]], m[p[11]]);
            g(&mut s, 2, 7, 8, 13, m[p[12]], m[p[13]]);
            g(&mut s, 3, 4, 9, 14, m[p[14]], m[p[15]]);
            r += 1;
        }
        s
    }

    #[inline]
    fn cv_from_compress(state: [u32; 16]) -> [u32; 8] {
        [
            state[0] ^ state[8],
            state[1] ^ state[9],
            state[2] ^ state[10],
            state[3] ^ state[11],
            state[4] ^ state[12],
            state[5] ^ state[13],
            state[6] ^ state[14],
            state[7] ^ state[15],
        ]
    }

    fn chunk_cv(
        input: &[u8],
        chunk_counter: u64,
        root_output: &mut Option<([u32; 8], [u8; 64], u32, u64, u32)>,
    ) -> [u32; 8] {
        let mut cv = IV;
        let mut block_index = 0usize;
        let blocks = if input.is_empty() {
            1
        } else {
            (input.len() + BLOCK_LEN - 1) / BLOCK_LEN
        };
        while block_index < blocks {
            let start = block_index * BLOCK_LEN;
            let end = core::cmp::min(start + BLOCK_LEN, input.len());
            let mut block = [0u8; 64];
            if end > start {
                block[..end - start].copy_from_slice(&input[start..end]);
            }
            let block_len = (end - start) as u32;
            let mut flags = 0u32;
            if block_index == 0 {
                flags |= CHUNK_START;
            }
            if block_index + 1 == blocks {
                flags |= CHUNK_END;
            }
            let prior = cv;
            let state = compress(&cv, &block, block_len, chunk_counter, flags);
            cv = cv_from_compress(state);
            if block_index + 1 == blocks {
                *root_output = Some((prior, block, block_len, chunk_counter, flags));
            }
            block_index += 1;
        }
        cv
    }

    fn parent_cv(left: &[u32; 8], right: &[u32; 8]) -> [u32; 8] {
        let mut block = [0u8; 64];
        let mut i = 0usize;
        while i < 8 {
            block[i * 4..i * 4 + 4].copy_from_slice(&left[i].to_le_bytes());
            i += 1;
        }
        i = 0;
        while i < 8 {
            block[32 + i * 4..36 + i * 4].copy_from_slice(&right[i].to_le_bytes());
            i += 1;
        }
        cv_from_compress(compress(&IV, &block, 64, 0, PARENT))
    }

    fn root_hash_from_output(
        cv: &[u32; 8],
        block: &[u8; 64],
        block_len: u32,
        counter: u64,
        flags: u32,
        out: &mut [u8; 32],
    ) {
        let s = compress(cv, block, block_len, counter, flags | ROOT);
        let mut i = 0usize;
        while i < 8 {
            out[i * 4..i * 4 + 4].copy_from_slice(&(s[i] ^ s[i + 8]).to_le_bytes());
            i += 1;
        }
    }

    fn root_hash_from_parent(left: &[u32; 8], right: &[u32; 8], out: &mut [u8; 32]) {
        let mut block = [0u8; 64];
        let mut i = 0usize;
        while i < 8 {
            block[i * 4..i * 4 + 4].copy_from_slice(&left[i].to_le_bytes());
            i += 1;
        }
        i = 0;
        while i < 8 {
            block[32 + i * 4..36 + i * 4].copy_from_slice(&right[i].to_le_bytes());
            i += 1;
        }
        root_hash_from_output(&IV, &block, 64, 0, PARENT, out);
    }

    pub(crate) fn hash(input: &[u8], out: &mut [u8; 32]) {
        let chunks = core::cmp::max(1, (input.len() + CHUNK_LEN - 1) / CHUNK_LEN);
        let mut stack = [[0u32; 8]; 54];
        let mut stack_len = 0usize;
        let mut chunk = 0u64;
        while (chunk as usize) < chunks {
            let start = chunk as usize * CHUNK_LEN;
            let end = core::cmp::min(start + CHUNK_LEN, input.len());
            let part = &input[start..end];
            let mut output = None;
            let cv = chunk_cv(part, chunk, &mut output);
            let mut cv_work = cv;
            let mut total = chunk + 1;
            let final_chunk = (chunk as usize) + 1 == chunks;
            while total & 1 == 0 {
                if final_chunk && total == 2 {
                    root_hash_from_parent(&stack[stack_len - 1], &cv_work, out);
                    return;
                }
                stack_len -= 1;
                cv_work = parent_cv(&stack[stack_len], &cv_work);
                total >>= 1;
            }
            stack[stack_len] = cv_work;
            stack_len += 1;
            if stack_len == 54 {
                unreachable!("BLAKE3 tree depth exceeds supported limit");
            }
            chunk += 1;
        }

        if chunks == 1 {
            let mut output = None;
            let _ = chunk_cv(input, 0, &mut output);
            let (prior, block, block_len, counter, flags) = output.unwrap();
            root_hash_from_output(&prior, &block, block_len, counter, flags, out);
            return;
        }

        let mut right = stack[stack_len - 1];
        stack_len -= 1;
        while stack_len > 0 {
            stack_len -= 1;
            let left = stack[stack_len];
            if stack_len == 0 {
                root_hash_from_parent(&left, &right, out);
            } else {
                right = parent_cv(&left, &right);
            }
        }
    }
}

const SETUP_DATA_TYPE: u32 = 0x4c55_4e41; // "LUNA"
const SETUP_PROGRESS_TYPE: u32 = 0x4c55_4e50; // "LUNP"
const PROGRESS_MAGIC: [u8; 8] = *b"LUNAPR01";
const HANDOFF_MAGIC: u64 = ux64_from_bytes(*b"LUNAHD01");
const HANDOFF_MAJOR: u16 = 1;
const HANDOFF_HEADER_SIZE: usize = 84;
const HANDOFF_ALIGNED_HEADER_SIZE: usize = 88;
const HANDOFF_MAX_SIZE: usize = 64 * 1024;
const SETUP_DATA_NODE_SIZE: usize = 16;
const RECORD_HEADER_SIZE: usize = 8;
const RECORD_ALIGN: usize = 8;
const RECORD_SYSTEM_PARTITION: u16 = 1;
const RECORD_DATA_PARTITION: u16 = 2;
const RECORD_SYSTEM_IMAGE: u16 = 3;
const RECORD_KERNEL_IDENTITY: u16 = 4;
const RECORD_LUNA_INIT_IMAGE: u16 = 5;
const RECORD_BOOT_MODE: u16 = 6;
const RECORD_BOOT_STATE: u16 = 7;
const RECORD_RECOVERY_DATA_IMAGE: u16 = 8;

static mut HANDOFF_PHYS: u64 = 0;
static mut HANDOFF_SIZE: u32 = 0;
static mut INIT_PHYS: u64 = 0;
static mut INIT_SIZE: u64 = 0;
static mut INIT_DIGEST: [u8; 32] = [0; 32];
static mut HANDOFF_VALID: bool = false;
static mut INIT_VALID: bool = false;
static mut PROGRESS_PHYS: u64 = 0;
static mut PROGRESS_ATTEMPT_ID: u64 = 0;

const fn ux64_from_bytes(bytes: [u8; 8]) -> u64 {
    u64::from_le_bytes(bytes)
}

#[inline]
unsafe fn read_u16(base: *const u8, offset: usize) -> u16 {
    u16::from_le(ptr::read_unaligned(base.add(offset).cast()))
}

#[inline]
unsafe fn read_u32(base: *const u8, offset: usize) -> u32 {
    u32::from_le(ptr::read_unaligned(base.add(offset).cast()))
}

#[inline]
unsafe fn read_u64(base: *const u8, offset: usize) -> u64 {
    u64::from_le(ptr::read_unaligned(base.add(offset).cast()))
}

#[inline]
fn range_ok(offset: usize, size: usize, total: usize) -> bool {
    offset <= total && size <= total - offset
}

#[inline]
unsafe fn clear_state() {
    HANDOFF_PHYS = 0;
    HANDOFF_SIZE = 0;
    INIT_PHYS = 0;
    INIT_SIZE = 0;
    INIT_DIGEST = [0; 32];
    HANDOFF_VALID = false;
    INIT_VALID = false;
    PROGRESS_PHYS = 0;
    PROGRESS_ATTEMPT_ID = 0;
}

#[inline]
unsafe fn unmap(base: *mut u8, len: usize) {
    bindings::early_memunmap(base.cast::<c_void>(), len);
}

unsafe extern "C" {
    fn memremap(phys: bindings::phys_addr_t, size: usize, flags: u64) -> *mut c_void;
    fn memunmap(addr: *mut c_void);
}

#[inline]
unsafe fn memremap_runtime(phys: u64, size: usize) -> *mut c_void {
    const MEMREMAP_WB: u64 = 1;
    memremap(phys as bindings::phys_addr_t, size, MEMREMAP_WB)
}

#[inline]
unsafe fn memunmap_runtime(addr: *mut c_void) {
    memunmap(addr);
}

#[unsafe(link_section = ".init.text")]
unsafe fn reject(base: *mut u8, len: usize) {
    clear_state();
    unmap(base, len);
}

unsafe fn update_progress(stage: u8, failure_code: u32) {
    if PROGRESS_PHYS == 0 {
        return;
    }
    let base = memremap_runtime(PROGRESS_PHYS, SETUP_DATA_NODE_SIZE + 32) as *mut u8;
    if base.is_null() {
        return;
    }
    let payload = base.add(SETUP_DATA_NODE_SIZE);
    ptr::copy_nonoverlapping(PROGRESS_MAGIC.as_ptr(), payload, PROGRESS_MAGIC.len());
    payload.add(8).write(1);
    let current = payload.add(9).read_volatile();
    if current != 0 && current != 255 && stage != 255 && stage < current {
        memunmap_runtime(base.cast::<c_void>());
        return;
    }
    payload.add(9).write_volatile(stage);
    (payload.add(12) as *mut u64).write_volatile(PROGRESS_ATTEMPT_ID);
    (payload.add(20) as *mut u32).write(failure_code);
    let attempt_id = PROGRESS_ATTEMPT_ID;
    memunmap_runtime(base.cast::<c_void>());
    kernel::pr_info!(
        "Luna: boot progress attempt={} stage={} failure={}\n",
        attempt_id,
        stage,
        failure_code
    );
}

unsafe fn verify_init_digest(phys: u64, size: u64, expected: &[u8; 32]) -> bool {
    if size == 0 || size > 64 * 1024 * 1024 || phys.checked_add(size).is_none() {
        kernel::pr_err!("Luna: init digest range rejected at {:#x}+{}\n", phys, size);
        return false;
    }
    let len = size as usize;
    let base = memremap_runtime(phys, len) as *const u8;
    if base.is_null() {
        kernel::pr_err!(
            "Luna: init digest early_memremap failed at {:#x}+{}\n",
            phys,
            len
        );
        return false;
    }
    let input = core::slice::from_raw_parts(base, len);
    let mut digest = [0u8; 32];
    blake3_verify::hash(input, &mut digest);
    memunmap_runtime(base as *mut u8 as *mut c_void);
    let mut diff = 0u8;
    let mut index = 0usize;
    while index < 32 {
        diff |= digest[index] ^ expected[index];
        index += 1;
    }
    diff == 0
}

#[unsafe(link_section = ".init.text")]
unsafe fn parse_handoff(phys: u64, node_len: u32) {
    kernel::pr_info!(
        "Luna: parse_handoff entered phys={:#x} len={}\n",
        phys,
        node_len
    );
    clear_state();

    let len = node_len as usize;
    if !(HANDOFF_HEADER_SIZE..=HANDOFF_MAX_SIZE).contains(&len) {
        kernel::pr_err!("Luna: handoff length rejected: {} bytes\n", len);
        return;
    }
    if phys.checked_add(len as u64).is_none() {
        kernel::pr_err!("Luna: handoff physical range overflow\n");
        return;
    }

    let base = bindings::early_memremap(phys as bindings::phys_addr_t, len) as *mut u8;
    if base.is_null() {
        kernel::pr_err!(
            "Luna: handoff early_memremap failed at {:#x}+{}\n",
            phys,
            len
        );
        return;
    }

    let magic = read_u64(base, 0);
    let major = read_u16(base, 8);
    let header_size = read_u32(base, 12) as usize;
    let total_size = read_u32(base, 16) as usize;
    let records_offset = read_u64(base, 36) as usize;
    let records_size = read_u64(base, 44) as usize;

    kernel::pr_info!("Luna: handoff raw magic={:#x} major={} header={} total={} records_off={} records_size={}\n", magic, major, header_size, total_size, records_offset, records_size);

    if magic != HANDOFF_MAGIC
        || major != HANDOFF_MAJOR
        || header_size != HANDOFF_HEADER_SIZE
        || total_size != len
        || records_offset != HANDOFF_ALIGNED_HEADER_SIZE
        || records_offset % RECORD_ALIGN != 0
        || !range_ok(records_offset, records_size, len)
        || records_offset.checked_add(records_size).is_none()
    {
        kernel::pr_err!(
            "Luna: handoff header rejected magic={:#x} major={} header={} total={} records_off={} records_size={} len={}\n",
            magic,
            major,
            header_size,
            total_size,
            records_offset,
            records_size,
            len
        );
        reject(base, len);
        return;
    }

    let records_end = records_offset + records_size;
    let mut offset = records_offset;
    let mut init_seen = false;
    let mut system_seen = false;
    let mut data_seen = false;
    let mut image_seen = false;
    let mut kernel_seen = false;
    let mut mode_seen = false;
    let mut state_seen = false;
    let mut recovery_seen = false;
    let mut boot_mode = 0u8;
    let mut init_digest = [0u8; 32];

    while offset < records_end {
        if records_end - offset < RECORD_HEADER_SIZE {
            reject(base, len);
            return;
        }

        let record_type = read_u16(base, offset);
        kernel::pr_info!(
            "Luna: handoff record off={} type={} size={}\n",
            offset,
            record_type,
            read_u32(base, offset + 4)
        );
        let record_size = read_u32(base, offset + 4) as usize;
        let payload = match offset.checked_add(RECORD_HEADER_SIZE) {
            Some(value) => value,
            None => {
                reject(base, len);
                return;
            }
        };
        if !range_ok(payload, record_size, records_end) {
            reject(base, len);
            return;
        }

        match record_type {
            RECORD_SYSTEM_PARTITION => system_seen = true,
            RECORD_DATA_PARTITION => data_seen = true,
            RECORD_SYSTEM_IMAGE => image_seen = true,
            RECORD_KERNEL_IDENTITY => kernel_seen = true,
            RECORD_BOOT_MODE => {
                if record_size != 1 {
                    reject(base, len);
                    return;
                }
                boot_mode = unsafe { *base.add(payload) };
                if boot_mode > 4 {
                    reject(base, len);
                    return;
                }
                mode_seen = true;
            }
            RECORD_RECOVERY_DATA_IMAGE => {
                if record_size < 40 {
                    reject(base, len);
                    return;
                }
                let version_len = read_u16(base, payload) as usize;
                let filename_len = read_u16(base, payload + 2) as usize;
                let strings = match payload.checked_add(40) {
                    Some(value) => value,
                    None => {
                        reject(base, len);
                        return;
                    }
                };
                let strings_len = match version_len.checked_add(filename_len) {
                    Some(value) => value,
                    None => {
                        reject(base, len);
                        return;
                    }
                };
                if strings_len > record_size - 40 {
                    reject(base, len);
                    return;
                }
                recovery_seen = true;
            }
            RECORD_BOOT_STATE => {
                if record_size != 24 {
                    reject(base, len);
                    return;
                }
                state_seen = true;
            }
            RECORD_LUNA_INIT_IMAGE => {
                if init_seen || record_size != 56 {
                    reject(base, len);
                    return;
                }
                let address = read_u64(base, payload);
                let size = read_u64(base, payload + 8);
                let digest_ptr = base.add(payload + 16);
                let mut digest = [0u8; 32];
                core::ptr::copy_nonoverlapping(digest_ptr, digest.as_mut_ptr(), digest.len());
                kernel::pr_info!(
                    "Luna: init record address={:#x} size={} end={:#x}\n",
                    address,
                    size,
                    address.saturating_add(size)
                );
                if address == 0 || size == 0 || address.checked_add(size).is_none() {
                    kernel::pr_err!("Luna: init record range rejected\n");
                    reject(base, len);
                    return;
                }
                INIT_PHYS = address;
                INIT_SIZE = size;
                INIT_DIGEST = digest;
                init_digest = digest;
                init_seen = true;
            }
            _ => {}
        }

        let next = match payload
            .checked_add(record_size)
            .and_then(|value| value.checked_add(RECORD_ALIGN - 1))
        {
            Some(value) => value & !(RECORD_ALIGN - 1),
            None => {
                reject(base, len);
                return;
            }
        };
        if next <= offset || next > records_end {
            reject(base, len);
            return;
        }
        offset = next;
    }

    if !(system_seen
        && data_seen
        && image_seen
        && kernel_seen
        && init_seen
        && mode_seen
        && state_seen
        && (boot_mode != 2 || recovery_seen))
    {
        kernel::pr_err!(
            "Luna: handoff records rejected system={} data={} image={} kernel={} init={} mode={} state={} recovery={}\n",
            system_seen as u8,
            data_seen as u8,
            image_seen as u8,
            kernel_seen as u8,
            init_seen as u8,
            mode_seen as u8,
            state_seen as u8,
            recovery_seen as u8
        );
        reject(base, len);
        return;
    }

    // Digest verification is performed after the normal memory subsystem is
    // available. The early parser only validates the handoff structure and
    // records the init image metadata.
    let _ = &init_digest;

    HANDOFF_PHYS = phys;
    HANDOFF_SIZE = len as u32;
    HANDOFF_VALID = true;
    INIT_VALID = true;

    let init_phys = INIT_PHYS;
    let init_size = INIT_SIZE;
    kernel::pr_info!(
        "Luna: handoff v1 accepted: {} bytes, init {:#x}+{}\n",
        total_size,
        init_phys,
        init_size
    );

    unmap(base, len);
}

/// Parse the Luna `setup_data` node chain supplied by the Linux x86 boot protocol.
#[unsafe(link_section = ".init.text")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_boot_parse(setup_data_phys: u64) {
    clear_state();
    let mut phys = setup_data_phys;
    kernel::pr_info!("Luna: setup_data head={:#x}\n", setup_data_phys);

    for index in 0..1024 {
        if phys == 0 {
            kernel::pr_info!("Luna: setup_data chain ended at node {}\n", index);
            return;
        }

        let node = bindings::early_memremap(phys as bindings::phys_addr_t, SETUP_DATA_NODE_SIZE)
            as *mut u8;
        if node.is_null() {
            clear_state();
            return;
        }

        let next = read_u64(node, 0);
        let node_type = read_u32(node, 8);
        let node_len = read_u32(node, 12);
        unmap(node, SETUP_DATA_NODE_SIZE);
        kernel::pr_info!(
            "Luna: setup_data node {} phys={:#x} type={:#x} len={} next={:#x}\n",
            index,
            phys,
            node_type,
            node_len,
            next
        );

        if node_type == SETUP_DATA_TYPE {
            let Some(payload_phys) = phys.checked_add(SETUP_DATA_NODE_SIZE as u64) else {
                clear_state();
                return;
            };
            parse_handoff(payload_phys, node_len);
            let handoff_valid = HANDOFF_VALID;
            let init_valid = INIT_VALID;
            let init_phys = INIT_PHYS;
            let init_size = INIT_SIZE;
            kernel::pr_info!(
                "Luna: handoff parse result valid={} init_valid={} init={:#x}+{}\n",
                handoff_valid as u8,
                init_valid as u8,
                init_phys,
                init_size
            );
        } else if node_type == SETUP_PROGRESS_TYPE && node_len >= 32 {
            let Some(payload_phys) = phys.checked_add(SETUP_DATA_NODE_SIZE as u64) else {
                clear_state();
                return;
            };
            let payload =
                bindings::early_memremap(payload_phys as bindings::phys_addr_t, 32) as *const u8;
            if !payload.is_null() {
                let mut magic = [0u8; 8];
                ptr::copy_nonoverlapping(payload, magic.as_mut_ptr(), magic.len());
                if magic == PROGRESS_MAGIC && *payload.add(8) == 1 {
                    PROGRESS_PHYS = phys;
                    PROGRESS_ATTEMPT_ID = read_u64(payload, 12);
                    let attempt_id = PROGRESS_ATTEMPT_ID;
                    kernel::pr_info!(
                        "Luna: boot progress found attempt={} at {:#x}\n",
                        attempt_id,
                        phys
                    );
                }
                unmap(payload as *mut u8, 32);
            }
        }

        phys = next;
    }

    if HANDOFF_VALID && INIT_VALID {
        update_progress(4, 0);
    } else if PROGRESS_PHYS != 0 {
        update_progress(255, 2);
    }
}

/// Return whether a valid Luna boot handoff and init image were parsed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_boot_available() -> bool {
    HANDOFF_VALID && INIT_VALID
}

/// Advance the RAM-resident boot progress record shared with `luna-boot.efi`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_boot_progress(stage: u8, failure_code: u32) {
    update_progress(stage, failure_code);
}

/// Verify the boot-reserved `luna-init` image after normal memory setup.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_init_verify() -> bool {
    if !HANDOFF_VALID || !INIT_VALID || INIT_PHYS == 0 || INIT_SIZE == 0 {
        return false;
    }
    let phys = INIT_PHYS;
    let size = INIT_SIZE;
    let expected = INIT_DIGEST;
    verify_init_digest(phys, size, &expected)
}

/// Return the physical address of the boot-reserved `luna-init` ELF image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_init_phys() -> u64 {
    INIT_PHYS
}

/// Return the exact byte size of the boot-reserved `luna-init` ELF image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_init_size() -> u64 {
    INIT_SIZE
}

/// Return the physical address of the validated Luna handoff object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_handoff_phys() -> u64 {
    HANDOFF_PHYS
}

/// Return the byte size of the validated Luna handoff object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn x86_luna_handoff_size() -> u32 {
    HANDOFF_SIZE
}
