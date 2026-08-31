# Project Luna — Implementation Audit 2026-08-31

## Scope

This audit compares the current `main` repository, the accepted Phase 1.6 decisions, RFC-0002, and the architectural Source of Truth.

## Current repository

`main` currently contains the 17 userspace/workspace crates plus the separate `boot/luna-boot` tree. `Cargo.toml` uses resolver 3 and lists the current crates, including `luna-root-mapping`, `luna-namespace`, `luna-state`, `luna-bundle`, the managers, runtimes and CLI.

PR #2 from Copilot is merged. It only changes `components/luna-bundle/src/lbp_v1.rs` and removes repeated parser reads/unsafe test unwraps without changing the LBP1 wire format.

## Source of Truth drift found

The current `docs/ARCHITECTURE.md` is authoritative for the historical/consolidated Phase 1.6 material, but it does not yet contain the accepted post-HZ clarification block and accepted RFC-0002 details as of 2026-08-31.

Because the file is large (~142 KB), the current post-HZ decisions are preserved in:

`docs/architecture/ARCHITECTURE-AMENDMENT-2026-08-31.md`

That amendment is the authoritative traceability bridge until the same material is safely folded into the main Source of Truth without losing its historical content.

The amendment records, among other points:

- `luna-system-runtime` replaces the generic/obsolete `lunad` terminology;
- `UserSession` is the combined user/session entity;
- applications see a normal Linux-compatible logical `/`;
- mappings are primarily file-oriented with explicit subtree mappings allowed;
- manifest mappings/capabilities are requests, while `luna-security` remains the authority;
- canonical DATA includes `system/config`, `system/state`, `users/<user>/{home,data,config}`, and `cache`;
- `luna-state` uses `redb` as its first durable backend;
- `luna-update-manager` is the mutation coordinator and domain managers retain ownership;
- `luna-namespace` contains Linux-specific namespace primitives;
- `/dev` remains filtered and authorization-controlled;
- internal IPC is Unix-domain sockets with a typed binary protocol;
- Recovery is distinct from Factory;
- RFC-0002 Bundle Format v1 is accepted with the fixed LBP1 container decisions;
- System Images remain direct `luna-X.Y.Z.squashfs` files and are never `.lbp`.

## Code review findings

### `luna-state`

The durable store is present and uses an atomic redb transaction for mutations plus global revision. This matches the accepted state contract. The API intentionally remains synchronous while higher layers may orchestrate asynchronously.

Remaining gap: system/runtime managers are not yet wired to persistent state as their authoritative implementation state.

### `luna-update-manager`

The coordinator and rollback prototype exist.

Important hardening gap: `prepare()` currently invokes the domain backend before persisting the durable prepared journal entry. The accepted transaction model requires intent/checkpoint state to be durable before physical mutation wherever possible. This should be corrected before connecting real domain backends.

Important recovery gap: interruption reconciliation is conservative because the current journal does not persist the exact set of successfully applied operations/checkpoints. A future implementation must persist per-step progress before claiming precise resume/rollback semantics.

### `luna-app-runtime`

The runtime tracks ApplicationInstance lifecycle and validates the mapping/bundle contract. It can prepare a namespace for an existing instance.

Important integration gap: the final authorization decision is not yet a mandatory input to the namespace-preparation path. The production path must enforce:

```text
manifest request
  ↓
mapping plan
  ↓
luna-security authorization
  ↓
namespace materialization
  ↓
exec
```

The existing `authorize()` method is a contract boundary, not yet complete end-to-end enforcement.

### `luna-root-mapping`

The component correctly keeps logical/physical paths separate and supports exact file mappings plus explicit subtree mappings.

The implementation still uses an in-memory deterministic materialization description. That is appropriate for the domain layer. Linux mount mechanics remain in `luna-namespace`.

### `luna-namespace`

The current backend uses real Linux mount namespace, bind mount, proc/sysfs/tmpfs and chroot primitives.

The current implementation has an architectural/implementation issue that must be fixed before treating logical-root materialization as reliable: it read-only bind-mounts the complete base root and then attempts to create missing mapping targets inside that read-only mount. This can fail for mappings whose logical target does not already exist in the base image.

The next implementation should use a writable composition layer (for example overlayfs over a read-only System Image lower layer) or another equivalent design, while keeping the final logical root conventional and keeping the System Image immutable.

### `luna-bundle`

The LBP1 codec is present and the Copilot cleanup is merged. The accepted format is represented by the current reader/writer boundary and RFC-0002.

One semantic naming mismatch still needs resolution in code/docs: the accepted manifest examples use `[[dependency]]`, while the Rust structure is named `dependencies` and TOML serialization therefore currently uses `[[dependencies]]`. The accepted RFC should use one canonical spelling, and the reference codec must match it. This is a conformance issue, not a reason to silently change the format.

The signature section is currently represented as raw optional bytes; full Ed25519 verification/trust binding is not yet production-complete.

## CI status

PR #2 is merged and its purpose was limited to Clippy cleanup. The repository should continue to use CI as the final truth for `cargo check`, `cargo test`, Clippy, release build, and the separate UEFI build.

## Next implementation order

1. Fix the logical-root composition so read-only System Image content and writable runtime overlays can coexist safely.
2. Make Security authorization a mandatory input to writable/device namespace materialization.
3. Connect `luna-app-runtime` process creation/supervision without moving that ownership into `luna-namespace`.
4. Make update preparation durable-before-mutation and persist exact applied-step state for interruption reconciliation.
5. Resolve the canonical manifest dependency field spelling and add parser conformance tests.
6. Add complete LBP1 integrity/signature tests and implement the Ed25519 verification boundary.
7. Integrate durable state with system/runtime ownership.
8. Only then proceed to System Image/kernel specification and production trust/boot-state work.
