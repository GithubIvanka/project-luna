# Alpine Foundation Experiment

## Goal

Build Luna from a known-working Alpine userspace outward instead of debugging the
entire custom boot stack at once.

This branch is an experimental control line. It does not replace the canonical
Luna architecture and must not change `develop`.

## Baseline

The first control artifact is OneFileLinux v0.3.2, an Alpine-based single-file
UEFI/Linux image. Its published artifact is a Linux bzImage with the Alpine
root filesystem embedded through the kernel initramfs mechanism.

The production foundation target is modern Alpine Linux v3.24.2 x86_64 with
musl. The old OneFileLinux kernel/userspace are reference material, not Luna
code to copy wholesale.

## Boot ladder

1. UEFI executes a single EFI-stub kernel image.
2. Stock Linux kernel initializes hardware and the built-in early userspace.
3. Alpine PID 1 reaches a usable shell/console.
4. We record the exact device, mount, init and process state.
5. Replace only the kernel command line policy.
6. Replace only Alpine init/OpenRC with a minimal Luna PID 1.
7. Reproduce the same successful storage layout.
8. Replace Alpine device handling with `luna-device-manager`.
9. Add the Luna system runtime.
10. Add native graphical login and compositor only after the text boot remains stable.

Every step gets its own boot test and a known-good checkpoint.

## Rules

No more than one foundational boot subsystem is replaced per step.
A failed step rolls back to the previous checkpoint instead of adding another
workaround on top of it.

The experiment keeps a serial boot transcript and, where relevant, a display
capture for every checkpoint.
