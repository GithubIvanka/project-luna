# Alpine 3.24 Boot Analysis and Luna Transfer Plan

## Scope

This document belongs to the alpine-foundation experiment. Alpine Linux is the
userspace reference; the bootloader and kernel under test are Project Luna.

Baseline: Alpine Linux v3.24.2 x86_64.

## Local source pack

    dist/sources/reference/alpine-3.24/
    ├── aports-3.24/
    ├── mkinitfs-3.14.1/
    ├── openrc-0.63.2/
    ├── busybox-1.37.0/
    ├── apk-tools-3.0.8/
    ├── alpine-conf-3.22.0/
    ├── release/
    │   └── alpine-minirootfs-3.24.2-x86_64.tar.gz
    └── SOURCE-MANIFEST.txt

These are reference sources and artifacts, not Luna runtime dependencies.

## What Alpine does before OpenRC

The central implementation is mkinitfs-3.14.1/initramfs-init.in.

Early userspace:
1. Creates the early boot namespace.
2. Installs and uses BusyBox utilities.
3. Mounts sysfs, devtmpfs (or tmpfs fallback), proc and devpts.
4. Creates /dev/shm.
5. Parses the kernel command line.
6. Loads required drivers and modules.
7. Handles optional crypt, RAID, LVM, NBD, ZFS, networking and special media.
8. Uses nlplug-findfs for block-device uevents, coldplug, LABEL/UUID/PARTUUID
   resolution and root discovery.
9. Mounts the selected root at /sysroot.
10. Moves needed mounts into the new root.
11. Executes switch_root.
12. Diskless and netboot paths can construct the root and install packages
    before the same final transition.

## Alpine init is the normal userspace boundary

The Alpine 3.24.2 minirootfs contains:

    /sbin/init -> /bin/busybox

Its /etc/inittab starts:

    /sbin/openrc sysinit
    /sbin/openrc boot
    /sbin/openrc default

The clean no-initramfs experiment is therefore:

    UEFI
      ↓
    luna-boot
      ↓
    Luna kernel
      ↓
    kernel mounts ALPINE-ROOT using root=PARTUUID
      ↓
    /sbin/init -> BusyBox
      ↓
    OpenRC
      ↓
    Alpine userspace

There is no second root and no root transition.

## Functional transfer matrix

| Alpine function | Reference | Current experiment | Luna destination |
|---|---|---|---|
| EFI/kernel loading | firmware + distro tooling | luna-boot | luna-boot |
| Root identity | root= + nlplug-findfs | fixed PARTUUID fixture | boot target/handoff |
| Device coldplug | netlink + sysfs trigger | critical devices built-in | kernel + luna-device-manager |
| Module loading | mdev + kmod | boot-critical drivers built-in | kernel; optional provider later |
| Filesystem support | initramfs modules | ext4 built into Luna kernel | kernel |
| /dev | initramfs devtmpfs | CONFIG_DEVTMPFS_MOUNT=y | kernel/bootstrap contract |
| /proc | initramfs then moved | Alpine/OpenRC | later early Luna environment |
| /sys | initramfs then moved | Alpine/OpenRC | later early Luna environment |
| /dev/pts | initramfs then moved | Alpine/OpenRC | later early Luna environment |
| Root mount | initramfs mount /sysroot | Linux kernel root= mount | Linux kernel |
| Root transition | switch_root | removed | removed |
| Boot media scan | nlplug-findfs | not needed for fixture | luna-boot discovery |
| LUKS/RAID/LVM/ZFS | initramfs helpers | outside minimal fixture | dedicated Luna storage layer |
| modloop | Alpine initramfs | no initrd | not in this boot chain |
| package install | apk into new root | fixed root image | image build pipeline |
| service startup | OpenRC | unchanged | Alpine reference remains |

## Current finding

The first functionality to reproduce is root selection, not the entire
nlplug-findfs feature set.

For this QEMU fixture the Luna kernel already has the required block and ext4
drivers built in. Linux can therefore resolve root=PARTUUID, mount the selected
filesystem as the real root and execute its /sbin/init directly.

Alpine functionality is decomposed instead of copied wholesale:

    Alpine nlplug-findfs
        ├── device events / coldplug
        ├── module activation
        ├── storage identity resolution
        ├── filesystem discovery
        └── root availability
                ↓
    Luna
        ├── kernel built-ins for boot-critical hardware
        ├── luna-boot identity / target resolution
        ├── Luna handoff metadata
        └── future luna-device-manager

Optional Alpine facilities such as encrypted roots, mdraid, LVM, ZFS and
network roots will be analyzed separately and implemented only where the Luna
storage model requires them.

## Test ladder

Previous control:

    UEFI → luna-boot → Luna kernel → Alpine initramfs → switch_root → OpenRC

New checkpoint:

    UEFI → luna-boot → Luna kernel → Alpine root → /sbin/init → OpenRC

Success criteria:
- Luna kernel is still the booted kernel.
- Alpine 3.24.2 remains the userspace.
- QEMU has no -initrd.
- luna-boot does not load Alpine initramfs.
- Linux mounts the explicit Alpine root directly.
- Alpine /sbin/init starts without switch_root.
- no chroot or pivot_root occurs.
- OpenRC reaches the normal console/login path.

The previous initramfs test remains the rollback/control line.

## Next transfer sequence

After the direct-root checkpoint is stable:
1. move root identity/discovery into the Luna boot target/handoff;
2. implement native device event/coldplug behavior;
3. define boot-critical driver activation policy;
4. define early /dev, /proc, /sys, /dev/pts, /run contracts;
5. then analyze System Image and DATA attachment against the same boundary.

The goal is functional equivalence with the useful Alpine early-boot behavior,
not a copy of initramfs-init.in.
