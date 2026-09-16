---
name: luna-boot
description: Implement and debug the Luna UEFI to kernel to luna-init boot path and boot-target resolution.
---
# Luna Boot
Canonical chain: UEFI -> luna-boot.efi -> Linux kernel -> luna-init PID 1.
Resolve atomic targets as System Image -> compatible init -> compatible kernel.
System Images are `.squashfs`; init artifacts are versioned ELF `.init` files.
Preserve `LunaBootHandoffV1` and its integrity/compatibility semantics.
Keep detailed attempt progress in RAM; durable boot state changes only on meaningful events.
B at startup requests the boot/recovery menu; normal boot remains direct and quiet.
Preserve the direct initial-userspace path with luna-init as the only PID 1.
Test bootloader changes with the QEMU/OVMF harness before hardware deployment.
