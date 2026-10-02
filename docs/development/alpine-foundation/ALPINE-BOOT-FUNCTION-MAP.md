# Alpine 3.24 Early Boot Function Map

Reference implementation:
dist/sources/reference/alpine-3.24/mkinitfs-3.14.1/

## 1. initramfs-init.in function groups

The exact functions currently present in Alpine mkinitfs 3.14.1 include:

| Function | Observed role | Luna transfer |
|---|---|---|
| ebegin/eend | boot progress/error reporting | luna-boot / Luna early logging |
| recovery_shell | early emergency shell | experiment recovery path; not required for minimum boot |
| unpack_apkovl | load persistent Alpine overlay | later optional boot-media provider |
| get_fstab_mount_info | resolve fstab mount metadata | later storage/mount policy |
| rc_add | enable OpenRC services in generated root | image builder, not kernel boot |
| list_console_devices | enumerate consoles | luna-boot/kernel handoff if needed |
| detect_serial_consoles | identify serial consoles | boot policy / console handoff |
| setup_inittab_console | modify Alpine inittab for console | experiment compatibility layer |
| dmi_match | DMI-based policy | optional hardware policy |
| setconsole_serial | select serial console | kernel cmdline / luna-boot |
| ethernets | enumerate ethernet devices | future device manager |
| find_first_interface_up | choose usable interface | future network provider |
| configure_ip | DHCP/static early network setup | optional network-root provider |
| remount_fstab_entry | relocate/remount according to fstab | normal Alpine userspace; no /sysroot move |
| find_boot_repositories | locate Alpine boot repositories | optional diskless/netboot provider |
| setup_nbd | attach NBD root | optional network storage provider |
| setup_wireguard | early WireGuard network | optional network provider |
| rtc_exists | choose hardware/software clock | Alpine/OpenRC; not root selection |
| is_url | classify network resource | optional netboot provider |
| prepare_zfs_root | import/select ZFS root | optional ZFS provider |
| want_tiny_cloud | detect cloud boot policy | optional cloud provider |
| prepare_apkovl | locate/select APK overlay | optional diskless provider |

## 2. Main execution responsibilities

Beyond the named helper functions, the main script performs these operations:

### Early environment

- parses the kernel command line;
- creates the temporary early directory tree;
- installs/activates BusyBox;
- mounts sysfs;
- mounts devtmpfs, falling back to tmpfs;
- mounts proc;
- mounts devpts;
- creates dev/shm;
- prepares kmsg/error output.

### Driver preparation

- reads requested modules from the command line;
- loads root filesystem-related modules;
- loads loop/squashfs/simpledrm and requested storage drivers;
- relies on kmod and mdev when modules are not built in.

Luna transfer:

- make boot-critical controller/block/filesystem drivers built-in;
- keep optional module loading out of the minimum root path;
- later implement a Luna module policy around kmod semantics.

### Storage discovery

The main script invokes nlplug-findfs and waits for a requested root.

nlplug-findfs itself provides:

- Linux NETLINK_KOBJECT_UEVENT reception;
- coldplug replay;
- MODALIAS extraction;
- kmod module lookup/insert;
- block-device identification with blkid;
- LABEL matching;
- UUID matching;
- PARTUUID matching;
- persistent media scanning;
- optional mdadm/lvm/zpool/cryptsetup coordination;
- timeout/settling behavior.

Luna transfer:

- identity owned by luna-boot when possible;
- storage device events owned by future luna-device-manager;
- kernel dev_t/block references used after identity resolution;
- optional storage providers remain isolated.

### Root preparation

Alpine finally:

    mount selected filesystem at /sysroot
    move selected early mounts into /sysroot
    exec switch_root

Luna experiment:

    resolve selected root
    mount selected filesystem directly on /
    install covering mount as current VFS root
    continue into Alpine /sbin/init

The second root tree is not constructed.

## 3. nlplug-findfs state machine

The relevant state machine is:

    kernel uevent
       |
       v
    parse ACTION/SUBSYSTEM/DEVNAME/MODALIAS
       |
       +--> MODALIAS -> kmod
       |
       +--> block add/change
                |
                +--> blkid TYPE
                +--> LABEL
                +--> UUID
                +--> PARTUUID
                +--> boot media scan
                +--> mdadm/LVM/ZFS/cryptsetup
                |
                v
             root found

Before relying only on future events, Alpine explicitly replays uevents by
writing add to sysfs uevent files for selected bus/class paths.

Luna transfer should preserve this behavior because it handles the race between
driver registration and a userspace observer starting late.

The Luna implementation should expose this as a typed event stream, not as an
mdev compatibility interface.

## 4. mdev behavior

Alpine mdev.conf handles:

- MODALIAS-triggered module loading;
- persistent-storage helper;
- DRM card nodes;
- sound nodes;
- USB helpers;
- tty aliases;
- misc device nodes;
- virtio-port naming.

The direct-root experiment should not reproduce mdev.conf wholesale.

The useful Luna contracts are:

- device discovery;
- device identity;
- device node creation/permissions;
- subsystem-specific properties;
- consumer event delivery.

Device node policy belongs to the future Luna device manager and permission
boundary, while devtmpfs remains a kernel primitive.

## 5. OpenRC boundary

Alpine-baselayout builds the base filesystem tree and /etc/inittab.

Alpine's normal boot then uses:

    /sbin/init -> /bin/busybox
    /sbin/openrc sysinit
    /sbin/openrc boot
    /sbin/openrc default
    getty/login

The experiment does not replace this. It proves that Alpine normal userspace can
start when the root is prepared directly by the Luna kernel path.

## 6. What does NOT move into Luna

These remain Alpine compatibility behavior during this experiment:

- BusyBox userland utilities;
- Alpine /etc/inittab;
- OpenRC service definitions;
- normal getty/login;
- APK package management;
- Alpine rootfs image contents.

These are not part of early-root replacement.

## 7. What SHOULD move into Luna

### Loader

- disk/partition identity;
- boot target selection;
- kernel/init artifact verification;
- handoff records;
- root identity record.

### Kernel

- boot-critical storage/controller support;
- partition registration;
- early root identity resolution;
- direct real-root mount;
- devtmpfs bootstrap.

### luna-device-manager

- uevent listener;
- coldplug replay;
- sysfs inspection;
- device registry;
- stable DeviceId/VolumeId;
- device lifecycle events.

### Future storage provider

- LUKS;
- mdraid;
- LVM;
- ZFS;
- network block roots.

These are separate optional capabilities, not reasons to restore initramfs.

## 8. Acceptance philosophy

A transferred function is accepted only when:

1. the original Alpine behavior is identified in source;
2. Luna has a documented equivalent contract;
3. the equivalent is implemented in the correct lifetime layer;
4. the direct boot checkpoint passes;
5. the control initramfs path remains runnable.

The goal is to preserve needed behavior while removing the Alpine-specific
temporary-root architecture.
