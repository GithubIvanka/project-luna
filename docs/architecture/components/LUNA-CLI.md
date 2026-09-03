# `luna-cli`

**Status:** client boundary implemented at foundation level

## Purpose
Provide the user-facing `luna` command as a thin client over Luna backend operations.

## Owns
- command parsing and presentation;
- user-facing invocation of backend contracts;
- human-readable/structured output selection.

## Does not own
Domain state, update execution, application process lifecycle, authorization policy or direct mutation of lower-level storage that bypasses the owning backend.

## Contract
CLI commands must call the component that owns the operation. A CLI shortcut is not a new ownership boundary.

Long-running operations belong to backend Operation context and must survive CLI disconnect according to the operation contract.

## Dependencies
Backend manager/runtime contracts and shared domain values. The CLI may depend upward; lower-level components must not depend on it.

## Open
Complete command taxonomy, IPC transport and full operation presentation remain to be implemented.
