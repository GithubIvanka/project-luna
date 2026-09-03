# `luna-fs`

**Status:** implemented foundation

## Purpose
Provide low-level filesystem primitives and metadata without embedding Luna policy.

## Owns
- filesystem handles/primitives;
- metadata and filesystem errors;
- host-backed/test implementations where needed.

Current API direction includes `FileSystem`, `open`, `create`, `remove` and `metadata` operations.

## Does not own
- logical path mapping;
- authorization;
- application lifecycle;
- Bundle installation;
- configuration precedence;
- namespace policy.

## Dependencies
May use standard OS/filesystem primitives and `luna-common` where required. It must not depend upward on managers or runtimes.

## Contract
A successful filesystem operation means the underlying primitive succeeded. It does not imply that the caller was authorized in the Luna security model; callers must pass through the appropriate policy boundary.

## Integration
Used by higher-level mapping/storage components. Security and mapping decisions remain outside this crate.

## Open
Production backends, filesystem-specific optimizations and complete async integration remain implementation concerns unless separately accepted.
