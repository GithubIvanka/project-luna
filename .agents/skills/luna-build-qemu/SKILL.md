---
name: luna-build-qemu
description: Build Luna artifacts and validate the bootable PC image through QEMU before physical deployment.
---
# Luna Build and QEMU
Use the current project build scripts as the starting point; repair stale assumptions rather than duplicating them.
Build Luna userspace natively for musl; glibc is optional compatibility runtime for applications.
The PC image must contain EFI, ext4 LUNA-SYS, Btrfs LUNA-DATA, and SWAP.
System Images belong in `LUNA-SYS/images/`; versioned init artifacts in `cores/`; kernels in `kernels/`.
Remove stale legacy boot inputs and paths from builders and tests.
QEMU must reach kernel -> luna-init -> system runtime before hardware installation is attempted.
Capture serial/kernel logs on failure and iterate until the failure is explained and fixed.
Never overwrite a user's physical disk from an automated build/test step.
