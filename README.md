# Project Luna

Project Luna is a custom operating-system project built around the Linux kernel, with its own boot, immutable System Image, runtime, application and recovery architecture.

## Source of Truth

The architectural source of truth is [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Start there, then follow the linked detailed architecture, accepted-decision summary and component documents.

## Canonical boot chain

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
luna-init
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
luna-app-runtime
```

`luna-init` performs early bootstrap, remains PID 1 for the normal system lifetime, and starts `luna-system-runtime` as a child process.

## Canonical storage

```text
Disk
├── EFI
├── LUNA-SYS
├── LUNA-DATA
└── SWAP
```

`LUNA-SYS` is OS-managed. `LUNA-DATA` is persistent mutable storage.

A System Image is directly `luna-X.Y.Z.squashfs` with an adjacent `luna-X.Y.Z.toml` manifest. `.lbp` is the Luna Bundle Format and is unrelated to System Image storage.

## Runtime

```text
luna-init (PID 1)
└── luna-system-runtime
    └── UserSession
        └── luna-app-runtime
            └── ApplicationInstance
```

Application launch is authorization-first: Bundle → ApplicationPlan → MappingPlan → `luna-security` → AuthorizedApplicationPlan → trusted setup → `luna-namespace` → process.

Mount namespace isolation is mandatory. A PID namespace is not required for ordinary applications.

## Development

Luna components are written in Rust. The Luna kernel integration is maintained separately under `kernel/`.

Development changes follow [`docs/development/AI-DEVELOPMENT-RULES.md`](docs/development/AI-DEVELOPMENT-RULES.md).

## Archive

Pre-audit documentation is temporarily retained under `docs/archive/2026-09-14-pre-audit/`. It is non-normative reference material and must not be used to infer current architecture.
