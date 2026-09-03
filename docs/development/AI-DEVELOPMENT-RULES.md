# Project Luna — AI Development Rules

**Status:** development policy
**Authority:** derived from `docs/ARCHITECTURE.md`, accepted decisions and component contracts

This file exists specifically to prevent architecture drift during AI-assisted development.

## 1. Mandatory reading order

Before changing code in a component, the AI MUST inspect:

1. `docs/ARCHITECTURE.md`;
2. `docs/decisions/ACCEPTED-DECISIONS.md`;
3. the relevant component document in `docs/architecture/`;
4. relevant RFC/ADR documents;
5. the crate map/API contracts;
6. the current implementation and tests.

For a historical question, historical phase/archive documents may be consulted, but they do not override current accepted decisions.

## 2. No architecture hallucination

The AI MUST NOT invent:

- a new Luna component;
- a new crate;
- a new manager/runtime/session daemon;
- a new persistent directory;
- a new protocol;
- a new public API;
- a new ownership boundary

merely because implementation would be easier that way.

A Linux command, helper binary or third-party daemon is not automatically a Luna component.

## 3. Ownership is binding

The implementation must preserve the accepted ownership model.

```text
luna-system-runtime
├── UserSession A
│   ├── luna-app-runtime
│   └── GUI/Desktop session
└── UserSession B
    ├── luna-app-runtime
    └── GUI/Desktop session
```

In particular:

- `UserSession` is the session boundary;
- `luna-system-runtime` is the single system-wide runtime/supervisor;
- `luna-app-runtime` owns ApplicationInstance execution/lifecycle;
- `luna-app-manager` owns application management, not normal process execution;
- `luna-security` owns authorization policy;
- `luna-root-mapping` owns logical mapping semantics;
- `luna-fs` owns filesystem primitives;
- `luna-namespace` owns Linux namespace/materialization primitives;
- `luna-update-manager` owns update transaction execution;
- `luna-boot.efi` owns the UEFI boot boundary.

There is no `luna-session`, `luna-runtime`, `luna-run-session` or equivalent replacement component unless an explicit architecture decision creates one.

## 4. Accepted versus open

The AI MUST distinguish:

- accepted;
- implemented;
- integration-in-progress;
- planned;
- open/undecided.

Open decisions must not be silently converted into implementation facts.

If code requires an undecided detail, stop at the smallest boundary possible and document the question or use an already accepted compatible mechanism without redefining the architecture.

## 5. Change procedure

For every non-trivial change:

```text
read contract
  ↓
identify owner
  ↓
inspect current code/tests
  ↓
implement inside existing boundary
  ↓
add/update tests
  ↓
run formatting/checks/tests
  ↓
inspect diff for architecture drift
  ↓
commit
```

A component must not be refactored merely to make another component convenient.

## 6. Dependency rules

Dependencies must point downward toward already-defined contracts where possible.

A lower-level crate must not depend upward on a manager/runtime simply to avoid implementing a local boundary.

`luna-common` remains small. Do not move subsystem-specific concepts into it for convenience.

## 7. Documentation before architecture changes

If implementation reveals that an accepted contract is technically impossible or seriously defective, the AI MUST NOT silently replace it.

Required sequence:

```text
implementation evidence
      ↓
architecture question
      ↓
accepted decision / RFC / ADR
      ↓
document update
      ↓
implementation
```

## 8. Filesystem/storage rules

Never change the physical storage model casually.

Canonical installation areas are:

```text
EFI / SYSTEM / DATA / SWAP
```

System Images are `luna-X.Y.Z.squashfs` with adjacent manifests. `.lbp` is a separate Bundle transport format.

Never place System Images/kernels under DATA just because a tool expects a normal filesystem tree.

## 9. Runtime/session rules

Do not introduce wrapper components to perform work already owned by UserSession/system-runtime.

The graphical login flow is:

```text
luna-system-runtime
 ↓
UserSession (Authenticating)
 ↓
luna-login
 ↓
authentication
 ↓
UserSession (Active)
 ↓
niri-session
 ↓
niri
 ↓
Noctalia
```

The final authentication IPC/security integration is allowed to evolve only through the established contracts.

## 10. Honesty rule

Never claim that a subsystem is integrated because a placeholder, shell wrapper, configuration file or model exists.

Examples:

- packaging Yazi is not the same as using `yazi-core` as Luna Files' backend;
- having service TOMLs is not the same as fully wiring D-Bus providers;
- having a kernel build script is not the same as a proven end-to-end boot;
- having a login handoff file is not the final Luna authentication IPC architecture.

## 11. Testing

Tests must target ownership boundaries and failure cases, not only happy paths.

At minimum, changes should verify:

- invalid state transitions are rejected;
- authorization is not bypassed by mapping;
- incompatible kernel/image pairs are rejected;
- failed transactions do not partially apply;
- session lifecycle cannot skip authentication;
- process failures do not corrupt unrelated sessions.

## 12. Git/CI rules

Never claim CI is green without checking the actual current commit's workflow results.

When a CI failure is found:

1. identify the exact failing step;
2. inspect the log;
3. reproduce locally when practical;
4. make the smallest contract-preserving fix;
5. rerun CI.

Do not paper over failures by weakening tests or removing checks unless that change is itself architecturally justified.

## 13. Before proposing a new component

The AI must answer all of these first:

1. What state does the proposed component uniquely own?
2. What responsibility cannot be placed in an existing owner?
3. Why is a library/API boundary insufficient?
4. Which existing component loses no ownership by the split?
5. What dependencies are required?
6. What lifecycle does it own?
7. What accepted decision authorizes the boundary?

If these cannot be answered, do not create the component.
