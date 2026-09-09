# Luna kernel patch stack

This directory contains the minimal Luna-specific changes applied on top of the pinned upstream Linux source tree by `tools/build-luna-kernel.sh`.

The current upstream base is Linux 7.2.4. Patch ordering is lexical (`*.patch` sorted by filename).

The target architecture is intentionally narrow:

```text
Linux bzImage
    -> validate LunaBootHandoffV1
    -> validate LUNA_INIT_IMAGE
    -> launch memory-resident luna-init as PID 1
```

These patches must not turn the kernel into a second `luna-init`, implement System Image policy, or construct Luna's logical root. Those responsibilities belong to `luna-init` and later userspace components.

Luna-specific kernel logic is Rust-first. A minimal C ABI bridge is allowed where Linux's early C initialization path needs to call Rust code directly; the bridge must remain thin and contain no Luna policy or parsing logic.

## Patch boundaries

```text
0001-luna-kernel-rust-parser.patch
    Rust parser and exported C ABI symbols only

0002-luna-kernel-rust-build.patch
    x86 Kbuild / Makefile integration only

0003-luna-kernel-rust-bridge.patch
    C declarations, CONFIG_RUST stubs, and setup.c call only
```

Keep these boundaries independent. A parser error, build integration error, and C/Rust declaration error must be attributable to one patch rather than debugged inside a combined diff.

The current stack adds only the kernel-side handoff plumbing and structural direct-initial-userspace entry point. It does not execute `luna-init` yet. ELF loading must reuse the existing Linux execution machinery wherever practical rather than introducing a second ELF loader.
