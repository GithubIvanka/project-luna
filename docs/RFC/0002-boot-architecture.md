# RFC-0002 — Luna Boot Architecture

Status: Draft

## Summary

Luna uses a custom minimal UEFI bootloader (`luna-boot.efi`) to provide a fast, silent, and atomic boot experience. The architecture separates the bootloader, kernel, and OS image to allow independent updates and a dedicated recovery environment.

## Design Goals

- **Silent Boot:** No menus, no timers, no GRUB splash screen. Press power → Logo → Login.
- **Recovery First:** Holding `B` immediately enters Recovery without loading the main OS.
- **Atomic Updates:** The OS image (`luna-os.img`) is swapped entirely. No partial updates.
- **Independent Kernel:** Kernel updates do not require touching the `System` partition.
- **Minimal Recovery Kernel:** A dedicated, stripped-down kernel for system repair and rollback operations.

## Physical Layout

| Partition | Filesystem | Purpose |
| :--- | :--- | :--- |
| **EFI** | FAT32 | Bootloader, Kernels, Recovery, Config |
| **System** | SquashFS (RO) | Immutable OS Images (`luna-os-*.img`) |
| **Data** | Btrfs | Users, Apps, Drivers, Config, Cache |
| **Swap** | swap | Virtual Memory |

## EFI Partition Contents

```text
EFI/
└── LUNA/
    ├── luna-boot.efi # Minimal Rust-based UEFI loader
    ├── kernel-7.10.img # Main kernel (full drivers)
    ├── initramfs-7.10.img # Initramfs for main kernel
    ├── recovery.img # Minimal recovery kernel + tools
    └── boot.toml # Boot configuration (TOML)
```

## Boot Process

### Normal Boot (Default)

1. UEFI executes `EFI/LUNA/luna-boot.efi`.
2. `luna-boot` reads `boot.toml` to find the active kernel version.
3. Loads `kernel-X.Y.img` and `initramfs-X.Y.img`.
4. Passes control to the kernel with root=`/dev/mapper/luna_system` (where system is mounted from `luna-os-current.img`).
5. Kernel mounts `System` (as loop device) and `Data` (Btrfs).
6. Starts `luna-init` (PID 1).
7. `luna-init` starts `lunad`, which loads Bundles from `Data/system/`.

### Recovery Mode (Hold `B`)

1. UEFI executes `luna-boot.efi`.
2. Detects key `B` during early boot.
3. Launches `recovery.img` instead of main kernel.
4. Recovery mounts only `EFI` and `Data`. `System` is mounted read-only or as snapshot.
5. User can:
   - `luna rollback` — откатить версию ОС;
   - `luna remove driver` — удалить проблемный драйвер;
   - Check filesystem;
   - Edit `boot.toml`.
6. Reboot.

## Configuration (boot.toml)

```toml
[active]
kernel = "7.10"
os_image = "luna-os-2.0"

[recovery]
enabled = true
key = "B"

[timeout]
seconds = 0
