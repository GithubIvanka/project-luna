# Recovery DATA and Boot Lifecycle

**Status:** Accepted  
**Date:** 2026-09-13  
**Scope:** `System State`, Recovery Environment, DATA selection and boot fallback

## 1. System State

Project Luna models three persistent system roles:

```text
System State
├── current
│   ├── image
│   ├── init
│   └── kernel
│
├── factory
│   ├── image
│   ├── init
│   └── kernel
│
└── recovery
    ├── image
    ├── init
    ├── kernel
    └── data
```

`current` and `factory` are complete atomic targets backed by physical `LUNA-DATA`.
`recovery` is also a complete atomic boot target, but additionally references a versioned Recovery DATA Image.

A target is never assembled by mixing artifacts from different targets.

## 2. Recovery directory

Recovery artifacts have their own area on `LUNA-SYS`:

```text
LUNA-SYS/
├── config/
├── cores/
├── images/
├── kernels/
└── recovery/
    ├── recovery-X.Y.Z.squashfs
    ├── recovery-X.Y.Z.toml
    └── ...
```

The Recovery System Image is the SquashFS filesystem containing the Recovery system environment. Its manifest is the adjacent `recovery-X.Y.Z.toml` file.

Recovery System Images are versioned and independently updateable. They are not stored inside the Factory System Image and do not share the normal Image naming namespace.

The Recovery manifest follows the same two-stage compatibility model:

```text
Recovery System Image
        ↓ compatible luna-init
Recovery/selected luna-init
        ↓ compatible kernel
Selected kernel
```

The manifest identifies the Recovery Image and declares compatible `luna-init` versions. The selected `luna-init` manifest declares compatible kernels.

## 3. Recovery DATA Image

Recovery also has a separate versioned DATA Image. It represents the logical DATA layout used by Luna and is materialized into RAM for the Recovery Environment.

Conceptual content:

```text
Recovery DATA Image
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── config/
│   └── ...
├── users/
│   └── recovery/
│       ├── home/
│       ├── data/
│       └── config/
├── data/
└── cache/
```

The Recovery DATA Image contains Recovery-specific system programs, libraries, configuration and temporary user environment.

It is not selected through `luna-data.toml` and does not depend on physical DATA discovery.

## 4. DATA abstraction

Luna uses one logical DATA abstraction with different providers:

```text
DATA
├── PhysicalData
│   └── physical LUNA-DATA
│
└── VirtualData
    └── Recovery DATA Image materialized in RAM
```

For `current` and `factory`, the DATA provider is physical `LUNA-DATA`.

For `recovery`, `luna-init` first materializes the referenced Recovery DATA Image in RAM and presents that environment as DATA. The rest of the system uses the same DATA interface and does not need to distinguish normal execution from Recovery execution merely because DATA is virtual.

## 5. Recovery access to physical DATA

The physical `LUNA-DATA` becomes an object managed by Recovery tools rather than the DATA provider for the running Recovery system.

Recovery can:

- discover physical disks and `LUNA-DATA` candidates;
- inspect and validate physical DATA;
- select the intended DATA partition;
- repair or replace the DATA binding configuration;
- inspect system-managed DATA content;
- perform other diagnostics and recovery operations.

The Recovery environment therefore remains usable even when the normal physical DATA partition is absent, damaged or ambiguous.

## 6. Normal DATA selection

For `current` and `factory`, `luna-data.toml` binds the physical DATA partition:

1. Try the configured disk GUID + partition GUID pair.
2. If that pair is not present, scan all disks for valid `LUNA-DATA` partitions.
3. Exactly one valid candidate is accepted.
4. Zero candidates means DATA is missing.
5. Multiple candidates are ambiguous and must not be selected arbitrarily.

`luna-boot.efi` never modifies `luna-data.toml`.

## 7. Automatic Recovery transition

If normal boot cannot obtain an unambiguous physical DATA partition, `luna-boot.efi` automatically selects the Recovery target for a normal continuation request.

Recovery then boots using:

```text
Recovery System Image
        +
Recovery luna-init
        +
Recovery kernel
        +
Recovery DATA Image → RAM
```

The physical DATA lookup performed for normal boot is not reused as the Recovery DATA provider.

## 8. Boot attempt lifecycle

Boot state describes the current boot attempt and is separate from general persistent system state.

The boot flow distinguishes three levels of success:

```text
System Image
    ↓
Image initialization
    ↓
INIT initialization
    ↓
Kernel/runtime startup
```

Image or `luna-init` failures use soft fallback without reboot where the already loaded kernel remains usable.

A kernel panic is different: the machine reboots, and the next `luna-boot` invocation detects the incomplete attempt and selects the previous compatible kernel according to persistent boot state.

A boot attempt carries:

```text
attempt_id
selected image
selected init
selected kernel
image status
init status
kernel status
fallback_depth
previous_attempt_failed
failure_code
```

Only inter-boot state is persisted in the boot-state mechanism. Intermediate soft-fallback steps may remain in memory for the current boot attempt.

## 9. Atomicity rules

`current`, `factory` and `recovery` are atomic target definitions.

A fallback must resolve another complete compatible target rather than combining an Image, init or kernel from unrelated targets.

The Recovery target additionally carries the identity of the Recovery DATA Image that will be materialized in RAM.

## 10. Lifecycle of Recovery artifacts

Recovery System Images and Recovery DATA Images are independently versioned and may be updated over time without redefining the normal `current` target.

An updated Recovery environment may add diagnostics, repair utilities, filesystem tools, DATA selection tools, hardware diagnostics and other system maintenance functionality.
