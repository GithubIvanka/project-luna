# Project Luna — Main Branch Protection Decision

**Date:** 2026-09-01
**Status:** accepted repository policy
**Source of Truth:** `docs/ARCHITECTURE.md`

## Decision

`main` is the canonical Project Luna integration branch and must be protected at the GitHub repository level.

The repository ruleset for `main` should enforce:

- pull request required before merge;
- required CI status checks before merge;
- force-push blocked;
- branch deletion blocked;
- stale branch updates rejected when practical (`strict` status checking);
- conversation resolution required when review conversations exist.

At minimum, the required checks should cover the Rust workspace validation and the UEFI build. Once the PC-image workflow produces a stable CI check name, its image-build job should also be required before merging changes that affect the image.

Signed-commit enforcement is intentionally not part of the initial ruleset. It can be enabled later together with a repository-wide signing policy.

## Development flow

```text
main (protected)
   ↑
one short-lived development branch
   ↑
one coherent implementation pass
```

Direct development commits to `main` are not the normal workflow. `development` is the current working branch for the active implementation pass.

## GitHub configuration boundary

The repository cannot encode all branch-protection behavior in source files. The actual enforcement is a GitHub Ruleset / protected-branch configuration and must be enabled in repository settings.

The source repository records the intended policy here so the rule is auditable and survives changes of tooling or working branch.

## Rationale

Protection prevents accidental force-pushes, deletion and bypass of the review/CI gate on the canonical integration history. It is especially important for Luna because `main` is the baseline from which the System Image, bootloader, runtime and desktop integration are assembled.
