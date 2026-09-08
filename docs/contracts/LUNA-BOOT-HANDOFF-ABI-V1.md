# Luna Boot Handoff ABI v1

**Status:** Accepted  
**Date:** 2026-09-08  
**Scope:** `luna-boot.efi` → Linux kernel → `luna-init`

## 1. Purpose

Luna Boot Handoff ABI v1 defines the structured boot context produced by `luna-boot.efi` and consumed by the Luna Linux kernel integration and `luna-init`.

The target userspace boot chain is:

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

There is no separate initramfs userspace layer in the Luna boot architecture, no `switch_root` stage and no second system-init process.

## 2. Design principles

- The Linux x86 boot protocol remains the transport foundation.
- Luna-specific data is carried through `setup_data` records attached to `boot_params`.
- The handoff is a memory-resident boot object, never a file on SYSTEM, DATA or EFI.
- The ABI is fixed-header plus typed extensible records.
- Unknown record types may be skipped safely by consumers that understand the ABI version.
- `luna-boot` provides Luna boot selection facts; it does not duplicate kernel-owned hardware discovery.
- The handoff is not a replacement for the Linux kernel command line. Kernel-specific parameters remain kernel parameters; Luna state is structured data.

Linux x86 boot protocol 2.09+ provides the `setup_data` linked-list mechanism specifically to extend boot parameters beyond the fixed 4096-byte `boot_params` area. citeturn880881search1turn880881search2

## 3. Transport

`luna-boot.efi` constructs the Luna handoff in physical memory before `ExitBootServices` and links it into the Linux `setup_data` list.

Conceptually:

```text
boot_params
    │
    └── setup_data
          │
          └── LUNA_HANDOFF_V1 record
                └── typed Luna records
```

The handoff allocation must be treated as reserved boot data until the Luna kernel integration has consumed or explicitly retained it. The bootloader must not leave the handoff in memory that the kernel may immediately reclaim as ordinary free RAM.

## 4. ABI header

The logical v1 header is:

```text
u64 magic
u16 abi_major
u16 abi_minor
u32 header_size
u32 total_size
u32 flags
u32 reserved
u64 boot_attempt_id
u64 payload_offset
u64 payload_size
u8  checksum[32]
```

The actual on-wire Rust/C representation must use explicitly sized little-endian integer fields and static layout assertions. Native-language struct layout is not itself part of the ABI.

### Header semantics

`magic` identifies the Luna handoff record.

`abi_major` identifies breaking ABI revisions. A consumer must reject unsupported major versions.

`abi_minor` permits compatible extensions. A consumer may accept a newer minor version when all required records are understood and unknown records are safely skippable.

`header_size` is the number of bytes occupied by the fixed header.

`total_size` is the complete handoff size, including header and records.

`flags` contains only global handoff properties. Record-specific semantics belong in record flags.

`boot_attempt_id` identifies this concrete boot attempt across `luna-boot`, kernel and `luna-init`.

`payload_offset` and `payload_size` identify the typed record area relative to the beginning of the handoff.

`checksum` covers the ABI-defined handoff bytes with the checksum field zeroed during calculation.

## 5. Record format

Each record is encoded as:

```text
u16 type
u16 flags
u32 size
u8  payload[size]
```

Records are aligned according to the ABI alignment rule, currently 8 bytes. Padding is not semantic data and must be zeroed.

Unknown record types must be ignored after validating their bounds.

Malformed lengths, integer overflow, records extending beyond `total_size`, or impossible alignment must cause the handoff to be rejected.

## 6. Required v1 records

### `SYSTEM_PARTITION`

Identifies the immutable SYSTEM partition without binding Luna to Linux device names.

Discovery may use the filesystem label (`LUNA-SYSTEM` by policy) and other bootloader-side metadata. The handoff carries stable identity so `luna-init` can verify that the resolved Linux block device is the partition selected by `luna-boot`.

Payload:

```text
u8  disk_guid[16]
u8  partition_guid[16]
u16 label_len
u16 reserved
u8  label[label_len]
```

Linux device paths such as `/dev/sda2`, `/dev/nvme0n1p2` or similar are not part of the ABI.

### `DATA_PARTITION`

Identifies the persistent DATA partition using the same model:

```text
u8  disk_guid[16]
u8  partition_guid[16]
u16 label_len
u16 reserved
u8  label[label_len]
```

The filesystem label is a discovery/configuration identifier; the GUID pair is the stable identity used for boot-context verification. Duplicate labels must not silently resolve to an arbitrary partition.

### `SYSTEM_IMAGE`

Identifies the selected System Image.

Logical payload:

```text
u16 family_len
u16 version_len
u16 filename_len
u16 reserved
u8  manifest_identity[32]
u8  image_digest[32]
u8  strings[]
```

The strings contain the family identifier, semantic version and canonical image filename in the order described by the length fields.

The filename is relative to the canonical SYSTEM image area; an absolute physical block-device path is forbidden.

### `KERNEL_IDENTITY`

Identifies the kernel that is currently executing and the selected kernel artifact.

Logical payload:

```text
u16 release_len
u16 artifact_id_len
u16 format_len
u16 reserved
u8  kernel_digest[32]
u8  strings[]
```

The kernel image itself is already executing; this record is identity and provenance metadata for the running kernel/boot attempt.

### `BOOT_MODE`

Enumerates the boot path selected by `luna-boot`:

```text
NORMAL
DETAILED
RECOVERY
FACTORY
EXTERNAL
```

### `BOOT_STATE`

Contains only boot-selection/failure context required by the current attempt. It does not become a general state database and does not replace `luna-state`.

The exact compact payload is versioned with the Boot State Contract.

## 7. Integrity

The selected System Image record contains its expected content digest. The adjacent manifest identity is also included.

The kernel-facing handoff checksum protects the structure of the handoff itself. It does not by itself establish authenticity of the image or manifest.

System Image authenticity/trust remains a separate policy and update concern.

## 8. Hardware information boundary

Luna Handoff v1 does **not** duplicate generic hardware information already represented by the Linux x86 boot protocol or discovered by the kernel.

The bootloader continues to populate standard boot structures such as the Linux `boot_params` memory/firmware fields it owns. The Luna handoff carries only Luna-specific state.

This avoids turning the handoff into a second ACPI/E820/device-discovery protocol. Linux's x86 boot protocol already defines `boot_params` fields for data such as ACPI RSDP and an extensible `setup_data` list. citeturn856784search2turn880881search1

## 9.2 Memory ownership

The handoff memory is boot-reserved memory. `luna-boot` must allocate it before `ExitBootServices`, populate it completely, and publish its physical address through the Linux boot protocol.

After `ExitBootServices`, the bootloader performs no further UEFI allocation or filesystem activity.

## 10. Command-line relationship

The Luna handoff is the primary Luna-specific boot-state ABI.

The kernel command line remains available for:

- standard Linux kernel parameters;
- hardware/debug parameters;
- kernel configuration that Linux itself defines;
- temporary development diagnostics.

Luna must not require `luna.system_device`, `luna.data_device` or `luna.system_image` command-line parsing as the normal boot contract once ABI v1 is implemented.

This also avoids coupling Luna boot identity to whitespace-delimited strings and path syntax.

## 11. Initial userspace contract

The Luna kernel integration must make the following information available to `luna-init` before it executes:

```text
LunaBootHandoffV1
    ↓
validated LunaBootContext
    ├── SYSTEM partition identity
    ├── DATA partition identity
    ├── selected image identity + digest
    ├── running kernel identity + digest
    ├── boot attempt identity
    ├── boot mode
    └── boot state context
```

No initramfs archive is required to transport these values.

## 12. Kernel driver policy

Luna does not use initramfs to discover or load the drivers required to reach the SYSTEM partition, read the required filesystem, access the selected System Image and start `luna-init`.

The Luna kernel configuration must therefore build all boot-critical storage, bus, filesystem, crypto and hardware support required by the supported boot profiles directly into the kernel (`CONFIG_*=y`).

Optional post-boot drivers remain eligible for loadable modules.

Built-in firmware may also be used when a device requires firmware during boot and a filesystem-based lookup would otherwise reintroduce an early-userspace dependency. Linux supports built-in firmware with `CONFIG_EXTRA_FIRMWARE` and `CONFIG_EXTRA_FIRMWARE_DIR`. citeturn856784search0

## 13. Kernel modules

Each versioned kernel artifact owns its compatible module set.

Canonical layout:

```text
SYSTEM/kernels/
└── <kernel-id>/
    ├── bzImage
    ├── kernel.toml
    └── modules/
        └── lib/modules/<kernel-release>/...
```

The module set is tied to the kernel identity and must not be treated as a global shared pool.

The module tree may be materialized into the logical runtime filesystem after `luna-init` has constructed the system environment.

Where modules are allowed to load, Luna may enforce signed-module policy. Linux supports kernel-side module signature verification and a force-enforcement configuration. citeturn856784search4

## 14. Failure

A missing, malformed, incompatible or unverified required handoff record is a boot failure. `luna-init` must not guess a replacement image or device identity from unrelated paths when a required ABI record is invalid.

Boot fallback remains owned by the boot state/fallback mechanism. The handoff describes the attempt selected by `luna-boot`.

## 15. Evolution

ABI v2 or another major revision is required for incompatible structural changes.

Compatible additions should be expressed as new record types or optional fields covered by `abi_minor` rules.

Record type values are centrally allocated by the Luna boot contract. Reusing a retired type with different semantics is forbidden.

## 16. Non-goals

Luna Boot Handoff v1 is not:

- a filesystem image;
- an initramfs replacement filesystem;
- a second hardware-description standard;
- a general IPC protocol;
- a configuration database;
- a kernel-module loader protocol;
- an application/runtime contract.
