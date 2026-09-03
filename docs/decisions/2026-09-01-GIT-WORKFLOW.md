# Project Luna — Git Workflow Decision

**Дата:** 2026-09-01  
**Статус:** accepted development workflow  
**SoT:** `docs/ARCHITECTURE.md`

## 1. Canonical integration branch

`main` is the canonical integration branch and must remain the only permanently active integration line.

## 2. Short-lived development branches

Implementation work uses one short-lived branch from the current `main`, normally named:

```text
development
```

or a narrowly scoped replacement when parallel work is genuinely required.

Stacked `integration/*` branches are not the normal workflow.

## 3. Pull requests

One coherent implementation pass should normally produce one pull request against `main`.

Do not create a new PR merely to retarget or restack an existing change. Consolidate the change set first.

## 4. Superseded work

When a stacked PR is replaced by a consolidated change, close the superseded PRs and document that they were integrated into the canonical branch.

Obsolete branches may remain temporarily when the Git hosting integration cannot delete refs. They must not be treated as active development lines and should point at the current `main` once their contents are fully integrated.

## 5. Commit discipline

The repository should prefer small, coherent commits inside one development branch and one reviewable PR. A large architectural pass may contain several commits, but the dependency graph must remain understandable.

## 6. Architecture records

Any implementation decision that changes a previously implicit boundary must be recorded under `docs/decisions/` and reflected in the SoT, `STATUS.md` and/or `ROADMAP.md` as appropriate.

## 7. Current state

The Phase 2 stacked integration chain was consolidated into `main` on 2026-09-01. Obsolete integration branches were moved to the resulting `main` commit. New development continues from that consolidated state.
