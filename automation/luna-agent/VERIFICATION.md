# Luna Agent verification

Verification is a separate machine-run phase after every successful agent commit.
Checks are selected per task in `tasks.toml` and are allow-listed in
`verification.py`; task definitions cannot execute arbitrary shell commands.

## Checks

`git-diff-check` validates whitespace/errors with `git diff --check`.
`luna-init-test` runs the luna-init Cargo test suite.
`luna-init-static` validates the built luna-init ELF is static x86_64.
`luna-static` runs the repository static-policy checker when present.
`boot-ovmf` runs the Luna UEFI/OVMF integration harness.

A verification failure does not mark a task complete. The task remains
resumable, and the next AI turn receives the verification failure through the
persistent task state and log.

Verification is intentionally allow-listed rather than accepting arbitrary
commands from TOML, keeping the autonomous runner's execution surface small.
