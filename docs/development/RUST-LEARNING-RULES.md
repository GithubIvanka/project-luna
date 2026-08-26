# Project Luna — Rust Learning Rules

The project is being developed in Rust by a developer who already has some Python experience but is learning Rust.

## Rule 1 — Explain the code

When providing or creating Rust code for Luna, explain the important parts instead of treating the code as a black box.

At minimum, explain:

- what each important type represents;
- ownership and borrowing where relevant;
- `struct` / `enum` choices;
- `Result` / `Option`;
- traits when they are used;
- lifetimes when they materially affect the design;
- modules and crate boundaries;
- why a particular Rust mechanism was chosen.

## Rule 2 — Prefer learning over magic

Do not hide important Rust concepts behind unnecessarily clever abstractions.

If a concise but advanced implementation and a slightly longer educational implementation are both reasonable, prefer the educational one while the project is still being learned.

## Rule 3 — Relate Rust to Python when useful

When it clarifies a concept, compare it to the closest Python concept. The comparison is explanatory only; Rust's semantics remain primary.

## Rule 4 — Architecture before implementation

Do not write large amounts of implementation code before the corresponding Luna architecture and interface are agreed.

The existing Source of Truth explicitly prioritizes:

Architecture → RFC → Format → Interfaces → Prototype → Implementation → Integration

## Rule 5 — Preserve crate responsibility

Every Rust crate must have a clear reason to exist.

Existing workspace components include:

- `luna`
- `luna-common`
- `luna-log`
- `luna-fs`
- `luna-bundle`
- `luna-config`

New crates should be introduced only when they establish a real architectural boundary.
