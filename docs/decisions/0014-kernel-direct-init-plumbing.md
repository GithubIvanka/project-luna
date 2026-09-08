# ADR-0014 — Luna kernel direct-init plumbing

**Status:** Accepted
**Date:** 2026-09-08

## Decision

The Luna kernel integration receives `LunaBootHandoffV1` through Linux x86 `setup_data` and validates it before direct initial userspace launch.

The `LUNA_INIT_IMAGE` record identifies a memory-resident ELF image by physical address, byte size and BLAKE3-256 digest. The kernel must validate the memory range against the boot memory map and handoff bounds, verify the digest over the exact byte range, and validate ELF program headers before attempting execution.

The direct-init path is kernel-internal. Its responsibilities stop at:

1. locate and validate the Luna handoff;
2. locate and validate `LUNA_INIT_IMAGE`;
3. construct a read-only anonymous boot-context file object containing the validated handoff bytes;
4. install that object as FD 3 for the initial process;
5. enter the existing Linux process/ELF execution machinery for the supplied executable image, using the smallest safe integration point.

The kernel must not parse System Image policy, mount SquashFS as `/`, construct Luna logical root, manage users, or duplicate `luna-init` responsibilities.

## Initial implementation boundary

The first implementation is deliberately limited to the `x86_64` built-in Luna boot profile. It must fail closed when:

- the Luna handoff is absent or malformed;
- required records are missing or duplicated where uniqueness is required;
- `LUNA_INIT_IMAGE` is outside reserved/usable boot memory;
- the image digest does not match;
- ELF class, machine, file/program-header layout, LOAD ranges, entry mapping, PT_INTERP, or W^X requirements are invalid;
- FD 3 cannot be created read-only and positioned at offset zero.

No fallback to an initramfs is permitted by this path.

## Reuse of Linux ELF execution

The implementation should reuse established Linux ELF binary loading and credential/process setup mechanisms instead of introducing an independent ELF loader. Any memory-backed executable-object adapter exists only inside the kernel and must not create a userspace-visible filesystem path.

## Build policy

Luna-specific kernel changes are maintained as patches under `kernel/patches/` and applied by `tools/build-luna-kernel.sh` to the pinned upstream Linux release.
