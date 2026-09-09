# Luna kernel patch stack

This directory contains the minimal Luna-specific changes applied on top of the pinned upstream Linux source tree by `tools/build-luna-kernel.sh`.

The target architecture is intentionally narrow:

```text
Linux bzImage
    -> validate LunaBootHandoffV1
    -> validate LUNA_INIT_IMAGE
    -> launch memory-resident luna-init as PID 1
```

These patches must not turn the kernel into a second `luna-init`, implement System Image policy, or construct Luna's logical root. Those responsibilities belong to `luna-init` and later userspace components.

Luna-specific kernel logic is Rust-first. A minimal C ABI bridge is allowed where Linux's early C initialization path needs to call Rust code directly; the bridge must remain thin and contain no Luna policy or parsing logic.

Patch ordering is lexical (`*.patch` sorted by filename). Every patch must state its upstream base/version assumptions in its commit message or accompanying documentation.

The first patch adds only the kernel-side handoff plumbing and structural direct-initial-userspace entry point. ELF loading must reuse the existing Linux execution machinery wherever practical rather than introducing a second ELF loader.
