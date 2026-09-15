# luna-boot stages

This file describes the current bootloader stages only.

```text
1. UEFI entry
2. identify LUNA-SYS
3. read boot state/data binding
4. discover image/core/kernel candidates
5. resolve compatibility
6. optional B → Boot Menu
7. prepare Linux kernel and init memory
8. build LunaBootHandoffV1
9. write LunaBootAttempt
10. ExitBootServices
11. Linux kernel
12. luna-init
```

The loader must not add a userspace initialization layer between Linux and `luna-init`.

`/images`, `/cores`, `/kernels`, `/config` and `/recovery` are paths relative to the mounted `LUNA-SYS` filesystem root.

DATA is selected separately. Recovery may use a virtual Recovery DATA Image and therefore does not require the normal physical `LUNA-DATA` partition.

End-to-end bootability is valid only after QEMU/OVMF or real UEFI testing of the actual Luna kernel, artifacts and handoff.
