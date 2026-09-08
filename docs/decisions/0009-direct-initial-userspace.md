# ADR-0009 — Direct Initial Userspace и отсутствие initramfs

**Status:** Accepted  
**Date:** 2026-09-08

## Decision

Luna does not use an external initramfs as a normal boot layer.

The canonical boot chain is:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Luna Linux kernel
  ↓
luna-init (PID 1)
  ↓
luna-system-runtime
```

`luna-boot.efi` loads a statically linked `luna-init` ELF into a reserved physical-memory region before `ExitBootServices`. Luna Boot Handoff ABI v1 describes that memory object to the Luna-specific kernel integration.

The Luna kernel integration adds a direct initial-userspace execution path which consumes the memory-resident ELF and starts it as PID 1 without requiring an initramfs archive, a temporary root filesystem, `switch_root`, or `pivot_root`.

## 1. Responsibilities

### luna-boot.efi

`luna-boot`:

- discovers SYSTEM and DATA;
- selects and validates the System Image and compatible kernel;
- loads the Linux `bzImage`;
- loads the `luna-init` ELF into reserved boot memory;
- allocates and populates Luna Boot Handoff ABI v1;
- publishes the handoff through Linux `setup_data`;
- obtains the final UEFI memory map;
- calls `ExitBootServices`;
- transfers control to the Linux kernel.

`luna-boot` does not mount SquashFS and does not construct the Linux root filesystem.

### Linux kernel

The kernel:

- performs normal architecture and hardware initialization;
- initializes the built-in drivers required for Luna boot;
- discovers the Luna handoff from `setup_data`;
- validates the handoff and the `luna-init` memory object;
- preserves the boot-reserved memory until the initial userspace image has been consumed;
- directly creates the first userspace task from the memory-resident `luna-init` ELF;
- exposes the validated Luna boot context to `luna-init`.

The kernel remains the owner of CPU, memory, device, VFS, scheduler and other kernel subsystems. Luna does not replace Linux's hardware model with a second one.

### luna-init

`luna-init` is PID 1 and the first Luna userspace process.

It receives `LunaBootContext` supplied by the kernel and is responsible for turning the running kernel plus discovered storage into the Luna System Environment. It may mount SYSTEM read-only, access the selected SquashFS source, attach DATA according to the logical-root architecture, construct the RAM-backed logical root, and start `luna-system-runtime`.

## 2. Direct userspace execution

The direct-userspace path is intentionally a kernel feature rather than a userspace workaround.

The implementation must not emulate direct execution by silently creating an initramfs, ramfs root, temporary root directory, `pivot_root`, or `switch_root`.

The preferred implementation point is the Linux initial-userspace execution path around `kernel_init()` / `run_init_process()` and the ELF loading path. The exact kernel API and internal helper names are implementation details and are not part of the Luna ABI.

The required property is semantic:

```text
memory-resident signed/validated ELF
        ↓
Linux kernel ELF execution path
        ↓
userspace PID 1
```

The first implementation may use an internal kernel file/object representation solely to reuse existing ELF loading code. Such an internal representation is not a user-visible filesystem and must not become an architectural root layer.

## 3. luna-init artifact

`luna-init` is built as a statically linked `x86_64-unknown-linux-musl` executable.

The ELF must be independently runnable without shared-library lookup. A dynamically linked `luna-init` is not valid for the direct-userspace contract.

The image is loaded by `luna-boot` as an opaque byte range after validating:

- ELF64 little-endian format;
- x86_64 machine type;
- supported ELF version;
- program-header bounds;
- load-segment bounds;
- integer-overflow safety;
- entry point validity;
- required alignment constraints;
- content digest.

The kernel repeats security-critical validation before execution.

## 4. Memory ownership

The memory range containing `luna-init` is reserved boot memory. Linux must not place ordinary page allocator allocations over the range until the initial userspace image has been consumed.

The implementation must distinguish:

1. the physical bytes loaded by `luna-boot`;
2. the kernel's temporary execution/loading representation, if any;
3. the resulting userspace virtual memory mappings created by ELF loading.

After successful ELF loading, the original boot image may be released only according to the kernel implementation's lifetime rules and only after no required boot metadata or mapping still references it.

A failed initial-userspace load is fatal to the current boot attempt and must enter the Luna recovery/fallback policy rather than guessing a replacement userspace.

## 5. Handoff record

Luna Boot Handoff ABI v1 adds a required `LUNA_INIT_IMAGE` record:

```text
u64 physical_address
u64 size
u8  digest[32]
u32 flags
u32 reserved
```

The record identifies the exact memory object prepared by `luna-boot`.

`physical_address + size` must describe a range fully contained inside the bootloader-reserved memory map. The digest covers exactly the byte range supplied by the bootloader.

## 6. Trust boundary

The bootloader validates the image before handoff. The kernel validates the handoff bounds and ELF structure before execution. Authenticity is a separate trust decision from the structural handoff checksum.

For the initial implementation, the `luna-init` artifact identity must be bound to the selected boot target so that an unrelated ELF cannot be substituted silently.

## 7. Kernel driver policy

All storage, bus, block-device, filesystem, crypto and other support needed to:

```text
kernel
  ↓
find SYSTEM
  ↓
open ext4
  ↓
open selected SquashFS
  ↓
execute luna-init
```

must be built into the Luna kernel for the supported PC boot profile.

Optional drivers that are not required before `luna-system-runtime` may remain loadable modules and belong to the versioned kernel module set under `SYSTEM/kernels/<kernel-id>/modules/`.

Built-in firmware may be used for boot-critical hardware when necessary. Linux documents `CONFIG_EXTRA_FIRMWARE` for embedding firmware directly into the kernel. 

## 8. Why no initramfs

The classic initramfs model exists largely to provide a first userspace filesystem and to load drivers before the final root is available. Luna deliberately moves those boot-critical drivers into the kernel and moves the first userspace executable into a memory-resident boot object. This removes the separate initramfs userspace phase while preserving Linux's normal ELF execution and userspace transition semantics.

## 9. Relationship with System Image

The `luna-init` boot object is not the System Image and is not a second System Image.

System Image remains:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
```

The image provides the immutable system userspace source. `luna-init` is the minimal first userspace executable that prepares access to that source and establishes the Luna runtime environment.

## 10. Consequences

- No external initramfs is required for normal Luna boot.
- No `switch_root` or `pivot_root` is required by the new target architecture.
- `luna-init` is the first userspace process and owns PID 1.
- Boot-critical drivers are built into the kernel.
- Non-critical modules remain versioned with their owning kernel.
- The kernel must contain a small Luna-specific initial-userspace execution path.
- `setup_data` remains the transport for Luna boot metadata.
- The ABI remains independent from Linux command-line path parsing.
- The direct-userspace execution mechanism is a kernel implementation detail behind the Luna boot contract.

## 11. Implementation sequence

1. Define and implement the `LUNA_INIT_IMAGE` Handoff record.
2. Add a shared kernel/boot validation description for the `luna-init` ELF.
3. Implement the Linux kernel direct-memory initial-userspace execution path.
4. Remove the current initramfs dependency from the Luna boot test path.
5. Make `luna-init` consume `LunaBootContext` from the kernel instead of `luna.*` command-line parameters.
6. Replace the transitional `pivot_root`/bootstrap-copy implementation with the new final Luna System Environment construction.
7. Add privileged QEMU/OVMF tests covering the complete path.
