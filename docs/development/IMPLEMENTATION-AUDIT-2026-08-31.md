# Project Luna — Implementation Audit 2026-08-31

## Scope

This audit compares the current `main` repository, the accepted Phase 1.6 decisions, RFC-0002, and the architectural Source of Truth.

## Current repository

`main` contains the current 17 userspace/workspace crates plus the separate `boot/luna-boot` tree. `Cargo.toml` uses resolver 3 and lists the current crates, including `luna-root-mapping`, `luna-namespace`, `luna-state`, `luna-bundle`, the managers, runtimes and CLI.

PR #2 from Copilot is merged. It only changes `components/luna-bundle/src/lbp_v1.rs` and removes repeated parser reads/unsafe test unwraps without changing the LBP1 wire format. The PR is merged and its purpose is now part of `main` history.

## Source of Truth drift found

The current `docs/ARCHITECTURE.md` is the architectural authority and contains the consolidated Phase 1.1–1.6 material, but it still does not physically contain the accepted post-1.6 clarification block or the final RFC-0002 details.

The accepted post-1.6 decisions are therefore preserved in:

`docs/architecture/ARCHITECTURE-AMENDMENT-2026-08-31.md`

This amendment is the traceability bridge until the same material can be safely folded into the large Source of Truth file without overwriting its historical content.

## Important SoT corrections confirmed

The current Source of Truth already contains the correct major boundaries for the runtime, mapping, UserSession, security, materialization and Phase 1.6. It also explicitly says that older conflicting statements are superseded by the current consolidated section.

The amendment adds the later accepted implementation decisions that were missing from the body of the SoT, especially:

- `luna-system-runtime` is the single system-wide runtime/supervisor; `lunad` is not an architectural component;
- `UserSession` is the combined user/session entity;
- applications receive a normal Linux-compatible logical `/`;
- mappings are primarily file-oriented with explicit subtree mappings allowed;
- bundle mapping/capability declarations are requests, not grants;
- canonical DATA includes `system/config` and `system/state`;
- `redb` is the first durable state backend;
- update coordination remains in `luna-update-manager` while domain managers retain ownership;
- `luna-namespace` is the Linux-specific namespace/materialization boundary;
- `/dev` access remains filtered and policy-controlled;
- Recovery remains distinct from Factory;
- RFC-0002 Bundle Format v1 is accepted;
- System Images remain direct `luna-X.Y.Z.squashfs` files.

## Code review findings

### `luna-state`

The durable store is present and uses a single redb transaction for mutations and global revision. This matches the accepted synchronous storage contract.

Remaining gap: system/runtime managers are not yet wired to persistent state as their authoritative implementation state.

### `luna-update-manager`

The coordinator and rollback prototype exist.

Hardening gap: `prepare()` still calls `backend.prepare(plan)` before persisting the durable prepared journal entry. This must be changed before a real backend with side effects is connected, so that intent/state is durable before mutation wherever the operation requires it.

Recovery gap: the current journal does not persist the exact set of successfully applied operations. Precise interruption reconciliation therefore remains deferred.

### `luna-app-runtime`

The runtime tracks ApplicationInstance lifecycle and validates the bundle/mapping contract. It can prepare a namespace for an existing instance.

Integration gap: final authorization is not yet a mandatory input to the namespace-preparation API. The production path must enforce:

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

### `luna-root-mapping`

The component correctly keeps logical/physical paths separate and supports exact file mappings plus explicit subtree mappings.

The domain materialization description remains in-memory; Linux mount mechanics stay in `luna-namespace`.

### `luna-namespace`

The original implementation incorrectly mounted the complete base root read-only and then tried to create missing logical mount targets inside that read-only mount.

This was fixed on 2026-08-31 by switching logical-root composition to a writable OverlayFS upper/work layer over the immutable base root. The code still keeps mount mechanics separate from policy and mapping semantics.

Remaining integration work:

- authenticate/authorize writable mappings before applying them;
- populate filtered `/dev` from authorized device-manager resources;
- integrate process creation and cgroup setup;
- add privileged Linux integration tests rather than only message/path unit tests.

### `luna-bundle`

The LBP1 codec is present and the Copilot cleanup is merged.

One manifest naming issue remains to be reconciled: the accepted RFC example currently uses `[[dependency]]`, while the Rust serde field is `dependencies`, which serializes to `[[dependencies]]`. The canonical TOML field name must be made identical in RFC and reference codec before claiming full conformance.

The codec currently exposes optional signature bytes but does not yet perform the full Ed25519 verification/trust-binding flow.

## CI status

PR #2 is merged. The repository has CI for workspace check/test/Clippy/release and the separate UEFI target. Each code-changing pass must be validated by a fresh workflow result; stale CI results must not be used as evidence for newer commits.

## Next implementation order

1. Run the fresh CI for the namespace-overlay change and fix any actual compile/test/Clippy issues.
2. Make Security authorization a mandatory input to writable/device namespace materialization.
3. Connect `luna-app-runtime` to real child-process creation/supervision without moving lifecycle ownership into `luna-namespace`.
4. Make update journal state durable-before-mutation and persist per-operation/per-step progress.
5. Reconcile the canonical RFC-0002 dependency field spelling between accepted RFC text and Rust manifest serde model.
6. Add complete LBP1 malformed-input, determinism, integrity and signature coverage; implement the Ed25519 verification boundary.
7. Connect durable state to `luna-system-runtime` / domain-manager ownership.
8. Continue System Image/kernel specification, boot-success confirmation, device integration and end-to-end Linux/QEMU validation.
