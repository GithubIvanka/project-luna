# Project Luna — Component Development Template

Use this template when starting a dedicated component-development discussion.

## 1. Context to provide

```text
Read first:
- docs/ARCHITECTURE.md
- docs/decisions/ACCEPTED-DECISIONS.md
- docs/architecture/components/<COMPONENT>.md
- docs/development/API-CONTRACTS-1.6.md
- docs/architecture/CRATE-MAP.md
- relevant RFC/ADR files named by the component document
```

Then inspect the current component source and tests.

## 2. Required analysis

Before editing, state internally:

- exact ownership;
- explicit non-ownership;
- current implementation status;
- allowed dependencies;
- accepted invariants;
- open decisions;
- tests that prove the contract.

Do not fill an open decision with a guess.

## 3. Implementation sequence

```text
contract
 ↓
current implementation audit
 ↓
smallest compatible change
 ↓
unit tests
 ↓
integration tests
 ↓
format/check/clippy/build
 ↓
diff/architecture audit
 ↓
commit
```

## 4. New requirement

When a requested feature crosses a boundary:

```text
identify existing owner
       ↓
add API to existing owner if appropriate
       ↓
otherwise open architecture question
       ↓
ADR/RFC acceptance
       ↓
component map update
       ↓
implementation
```

Never create a new crate first and justify it afterwards.

## 5. Completion report

Every component-development task should report:

- files changed;
- contract preserved;
- tests added/updated;
- commands/checks run;
- CI result for the exact commit if available;
- remaining open work;
- any architecture question discovered.

Do not report an implementation as complete merely because it compiles.
