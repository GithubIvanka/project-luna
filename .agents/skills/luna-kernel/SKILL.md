---
name: luna-kernel
description: Work on Luna's Linux kernel overlay, direct luna-init execution, handoff validation, and Alpha hardware configuration.
---
# Luna Kernel
Keep Luna-specific kernel integration small and explicit.
Validate `LunaBootHandoffV1`, memory ranges, init ELF constraints, and init digest.
Use the existing `kernel/rust/luna_boot.rs` and `luna_exec.rs` path unless evidence requires redesign.
Preserve direct initial-userspace execution from the memory-resident luna-init handoff.
Alpha hardware priority: Intel UHD 630/i915, AX200 Wi-Fi/Bluetooth, r8169 Ethernet,
Intel HDA audio, USB xHCI, NVMe and SATA.
Prefer required early drivers built-in for Alpha; avoid premature module orchestration.
Ensure Btrfs, SquashFS, EFI/GPT and required namespaces/cgroups/security features are enabled.
