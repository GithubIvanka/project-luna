# Source implementation stages

The source implementation is organized around UEFI discovery and Linux handoff.

```text
GPT discovery
 → LUNA-SYS filesystem
 → manifest discovery
 → compatible target
 → kernel/init memory preparation
 → LunaBootHandoffV1
 → ExitBootServices
```

The active source paths are:

- `gpt.rs` — partition discovery;
- `filesystem.rs` / `ext4.rs` — LUNA-SYS access;
- `discovery.rs` — image/core/kernel discovery;
- `target.rs` — resolved target;
- `menu.rs` / `boot_key.rs` — exceptional Boot Menu;
- `kernel.rs` / `boot_params.rs` / `linux.rs` — Linux boot preparation;
- `handoff.rs` — Luna handoff;
- `boot_attempt.rs` — attempt marker;
- `external.rs` — UEFI external boot.

The implementation must remain aligned with `docs/architecture/BOOT-PATH.md`.
