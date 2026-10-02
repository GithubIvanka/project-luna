# Alpine → Luna Boot Transfer Plan

## Scope

This is the current alpine-foundation experiment.

Runtime under test:

    UEFI → luna-boot → Luna kernel 7.2.4 → Alpine Linux 3.24.2 userspace

The goal is to preserve the useful behavior of Alpine early boot while removing
the Alpine initramfs/root-switch mechanism from the boot chain.

This experiment does not replace the canonical Luna architecture.

## Source baseline

Local Alpine reference sources live in:

    dist/sources/reference/alpine-3.24/

Current pack:

- aports-3.24: 3.24-stable, f27459a477f6eda0f8951f4b98615972b85a1e41
- mkinitfs-3.14.1: fbd4083c0108b8d361d751ff812b26791c109a39
- openrc-0.63.2: b3a99e01c8bda7e5c2b9a8975a6d36ec3eff370e
- busybox-1.37.0: be7d1b7b1701d225379bc1665487ed0871b592a5
- apk-tools-3.0.8: 44dcdfc255c26a4e19a4a27f66a54169cce5ca72
- alpine-conf-3.22.0: c6002e5e150a9f17b2b24244114839b4b17bda51
- alpine-make-rootfs: 104d1f34c9a1cb599eff2123a6b6d042eea54577
- alpine-make-vm-image v0.13.4: ab659ff532b0e27684a08d11df116b9f7665081a
- musl 1.2.6: local upstream checkout
- util-linux 2.42.3: local upstream checkout
- kmod 34.2: local upstream checkout

Release artifacts:

- alpine-minirootfs-3.24.2-x86_64.tar.gz
- alpine-runtime-rootfs-3.24.2-x86_64.tar.gz
- corresponding verified sha512 files

Additional upstream storage/network source trees are added as needed.

## Alpine boot decomposition

Primary early-boot implementation:

    mkinitfs-3.14.1/initramfs-init.in

Its responsibilities are logically:

1. early filesystem and directory setup;
2. BusyBox and early utility availability;
3. /dev, /proc, /sys, /dev/pts and /dev/shm preparation;
4. kernel command-line parsing;
5. driver/module/firmware activation;
6. device and storage discovery;
7. optional encrypted/RAID/LVM/ZFS/NBD/network-root handling;
8. mounting the selected root at /sysroot;
9. moving required mounts into the selected root;
10. switch_root to the final userspace.

The important storage/device helper is:

    mkinitfs-3.14.1/nlplug-findfs/nlplug-findfs.c

It uses Linux NETLINK_KOBJECT_UEVENT, coldplug replay, MODALIAS/kmod loading,
block-device events and blkid metadata such as LABEL, UUID and PARTUUID. It can
also coordinate cryptsetup, mdadm, LVM and ZFS helpers.

Normal Alpine userspace starts separately:

    /sbin/init -> BusyBox
       -> OpenRC sysinit
       -> OpenRC boot
       -> OpenRC default
       -> getty/login

We keep this normal userspace unchanged during the experiment.

## Transfer matrix

| Alpine behavior | Current source | Luna destination | Status |
|---|---|---|---|
| EFI/kernel load | Alpine/firmware boot flow | luna-boot | working |
| root identity | root= plus nlplug-findfs | luna-boot + handoff + kernel lookup | in progress |
| block uevents | nlplug-findfs | Luna kernel + luna-device-manager | planned |
| coldplug replay | sysfs uevent writes | luna-device-manager | planned |
| MODALIAS module loading | kmod + mdev | built-in boot drivers first; module layer later | partial |
| boot filesystem drivers | mkinitfs feature lists | built into Luna kernel | working |
| /dev | devtmpfs | kernel CONFIG_DEVTMPFS | working |
| /proc | initramfs | Alpine/OpenRC on actual root | working |
| /sys | initramfs | Alpine/OpenRC on actual root | working |
| /dev/pts | initramfs | Alpine/OpenRC on actual root | working |
| /dev/shm | initramfs | Alpine/OpenRC on actual root | working |
| /run | normal Alpine/OpenRC | Alpine/OpenRC on actual root | working |
| root mount | initramfs /sysroot | Linux direct mount over / | working |
| root switch | switch_root | removed | working |
| chroot | not required by Alpine boot | not used | working |
| pivot_root | Linux root transition | not used in direct branch | working |
| BusyBox init | Alpine rootfs | unchanged Alpine | working |
| OpenRC | Alpine OpenRC | unchanged Alpine | working |
| getty/login | BusyBox/OpenRC | unchanged Alpine | working |

## Checkpoints

### B0 control

    UEFI → luna-boot → Luna kernel → Alpine initramfs
         → switch_root → OpenRC → login

Known-good rollback/control line.

### B1 direct real root

    UEFI → luna-boot → Luna kernel
         → Alpine EXT4 root mounted directly on /
         → /sbin/init → OpenRC → login

This is proven with the fixed /dev/sda3 fixture.

### B2 metadata root identity

Use:

    root=PARTUUID=<GPT partition GUID>

The direct-root path must wait until partition metadata has been registered.
Linux already provides early_lookup_bdev() and devt_from_partuuid(); we should
use that established kernel path rather than inventing a /dev/sda naming rule.

The kernel overlay has now been extended so direct-root mode invokes the normal
rootwait logic before the direct mount attempt.

### B3 Luna handoff root identity

After B2 is stable, remove the root identity as the authoritative command-line
input.

luna-boot already knows the selected GPT partition. Pass the selected root
partition identity in LunaBootHandoffV1 and let the kernel direct-root adapter
consume it.

The command line may remain as a debug/compatibility representation but is not
the source of truth.

### B4 native early device behavior

Transfer the useful behavior from nlplug-findfs:

    NETLINK_KOBJECT_UEVENT
        ↓
    event parser
        ↓
    coldplug replay
        ↓
    sysfs inspection
        ↓
    Luna device registry
        ↓
    storage discovery/event consumers

Do not reproduce the legacy Alpine /proc/sys/kernel/hotplug interface.

### B5 boot-critical driver policy

Boot-critical controller, block, partition and filesystem drivers remain built
into the Luna kernel for this experiment.

Alpine modloop is not part of the direct-root path.

Optional module loading is a separate future capability.

### B6 early environment contract

The Alpine userspace must see the normal ABI on the real root:

    /dev
    /proc
    /sys
    /dev/pts
    /dev/shm
    /run

We can move individual preparation steps into Luna-owned components only where
the behavior must exist before normal Alpine userspace. No second root is
introduced.

### B7 optional boot features

Only after the core boot path is stable:

- LUKS / cryptsetup
- mdraid / mdadm
- LVM
- ZFS
- NBD and network roots
- boot-media and APK overlay discovery

Each gets an isolated source-backed experiment.

## Rules for transferring Alpine behavior

For every feature:

1. identify the exact Alpine source;
2. describe its externally visible contract;
3. identify its required kernel primitives;
4. assign ownership to luna-boot, Luna kernel, luna-device-manager, early
   userspace or normal Alpine userspace;
5. implement the smallest equivalent contract;
6. add a QEMU checkpoint;
7. retain the previous checkpoint as rollback/control.

The objective is functional equivalence where useful, not a copy of Alpine's
initramfs architecture.

## Current status

B1 is demonstrated.

B2 is the current immediate implementation target. The current running kernel
rebuild contains the direct-root overlay with rootwait handling and will be used
for the PARTUUID test.

After B2 passes, B3 becomes the next substep. Only then should we move into the
native uevent/coldplug implementation.

