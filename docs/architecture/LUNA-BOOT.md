# Project Luna — `luna-boot.efi`

**Status:** accepted architecture; implementation in progress
**Boundary:** UEFI boot component, outside ordinary userspace workspace

## 1. Responsibility

`luna-boot.efi` owns functionality that belongs directly to the UEFI boot flow:

- discovering Luna System Images and kernels on SYSTEM;
- reading image manifests and boot metadata;
- deciding which compatible kernel/image pair to attempt;
- opening the boot menu only when the boot key is requested;
- loading the selected Linux kernel and required boot parameters;
- performing boot-time fallback according to persistent boot state;
- handing control to the Linux kernel.

It does not own userspace application lifecycle, UserSessions, application authorization or update transactions.

## 2. Boot chain

```text
UEFI
 ↓
luna-boot.efi
 ↓
compatible Linux kernel
 ↓
Luna System Image
 ↓
minimal RAM logical-root environment
 ↓
luna-system-runtime
 ↓
UserSession
```

## 3. Normal boot

Normal boot is immediate and quiet. There is no persistent menu delay.

```text
B pressed?
├── no  → select default compatible pair → boot
└── yes → show Boot Menu
```

The exact key-sampling window is an implementation detail of the UEFI loader; it must preserve the no-delay normal path.

## 4. Boot Menu

The accepted menu model contains:

1. **Continue to Luna** — boot the selected/default compatible image and kernel.
2. **Verbose Boot** — boot while exposing diagnostic boot information.
3. **System Image selection** — list available normal System Images and show only kernels compatible with the selected image.
4. **Recovery Environment** — boot the dedicated recovery environment.
5. **Factory Environment** — boot the immutable factory System Image + factory kernel pair.
6. **Boot from USB / External Device** — hand boot selection to an external bootable device.

The menu is an exception path, not the normal UI.

## 5. Image selection

`luna-boot` reads all discoverable System Images from SYSTEM and their adjacent manifests.

Default selection is the highest usable version according to the System Image version policy, not merely the newest filename.

For every image, the loader must filter kernels through explicit compatibility metadata. It must not present an arbitrary kernel/image pair simply because both files exist.

## 6. Fallback

Fallback is layered:

```text
selected image/kernel
      ↓ fail to start
compatible previous choice
      ↓ still unavailable
Recovery / Factory according to failure class
```

A System Image start failure should try a previous usable choice without reboot where technically possible. A kernel-level failure may require a reboot before another kernel/image pair can be selected.

Factory is the final known-good pair and is never removed by ordinary lifecycle management.

## 7. Boot state

Boot state is separate from System State and Recovery State.

It must be changed only on relevant events such as a successful selection/commit or a confirmed failure. Ordinary successful boots must not rewrite persistent boot state unnecessarily.

The accepted retention direction is to preserve the current usable choices and previous fallback choices; exact retention counts belong to the boot-state implementation contract, not this overview.

## 8. Responsibility boundary

`luna-boot` stops being responsible for the system once the Linux kernel has been handed control with the selected boot parameters and System Image context.

After that boundary:

```text
kernel / root mapping / system runtime
```

own normal userspace initialization.

`luna-boot` must not become a second init system or a general recovery manager.

## 9. Security and integrity

The loader must validate image/kernel metadata and reject malformed paths, incompatible metadata and invalid boot structures. Trust/signature semantics that require userspace policy must not be invented inside the loader.

## 10. Implementation status

The dedicated boot project already has UEFI/Linux boot protocol handling and has demonstrated kernel loading through a test init to `sh`. The remaining work is to harden the real System Image/kernel discovery, compatibility, fallback and final handoff against the accepted contract.
