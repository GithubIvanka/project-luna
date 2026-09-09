# Luna kernel integration

Project Luna builds against the pinned Linux source tree selected by
`tools/build-luna-kernel.sh`.

The kernel integration deliberately does not depend on fragile line-context
patch files. The build tool applies a deterministic source overlay:

```text
kernel/rust/luna_boot.rs
        ↓
tools/apply-luna-kernel-overlay.sh
        ↓
arch/x86/kernel/luna_boot.rs
arch/x86/kernel/Makefile
arch/x86/kernel/setup.c
```

The overlay fails closed when an expected Linux insertion point is missing or
an existing Luna source file differs from the repository copy. This is
intentional: an upstream kernel layout change must be handled explicitly
rather than silently applying with fuzz or changing the wrong location.

## Kernel-side boundary

The Rust component parses the LunaBootHandoffV1 `setup_data` node and exposes a
small C ABI for later kernel startup integration. The early x86 C bridge only
passes `boot_params.hdr.setup_data` into Rust; it contains no Luna parsing or
policy.

The parser validates the ABI major version, fixed header size, payload bounds,
record bounds, required record presence, `LUNA_INIT_IMAGE` structure and
integer-overflow conditions. Unknown record types remain skippable after their
bounds have been validated.

## Scope

This layer does **not** implement the full userspace bootstrap itself. Direct
execution of the validated memory-resident `luna-init` image and installation
of the mandated boot-context FD 3 remain kernel startup work behind this ABI.

The target architecture remains:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Luna Linux kernel
  ↓
luna-init (PID 1)
  ↓
luna-system-runtime
```

No separate initramfs userspace layer is introduced by this integration.
