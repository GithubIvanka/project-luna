---
name: luna-rust
description: Follow Project Luna Rust workspace conventions and safely implement, test, and refactor native musl components.
---
# Luna Rust
Read the local crate and its contracts before editing public APIs.
Prefer explicit types, clear errors, small modules, and tests for behavior changes.
Use workspace dependencies consistently; do not introduce duplicate crates without reason.
Target musl for native Luna userspace components.
Keep unsafe code narrowly scoped and document its safety invariants.
Run focused tests first, then the workspace checks relevant to the change.
Do not hide build failures behind disabled checks or weakening compiler/linter settings.
Keep compatibility code isolated from the native Luna path.
