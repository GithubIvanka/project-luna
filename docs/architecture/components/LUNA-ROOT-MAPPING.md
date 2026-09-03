# `luna-root-mapping`

**Status:** implemented contract; production integration incomplete

## Purpose
Construct validated logical filesystem mappings for a namespace.

## Owns
- `LogicalPath` / `PhysicalPath` semantics;
- mapping declarations and validated plans;
- conflict detection;
- mapping state for an application/security context.

## Does not own
- authorization;
- raw filesystem I/O;
- Linux namespace creation;
- application lifecycle;
- user/session management.

## Mapping model

Mappings are semantic and policy-controlled. File mappings are the default granularity; explicit subtree mappings are allowed for semantic classes such as shared library/resource trees.

The usual resource lookup layering is user → application → system where that resource class supports those layers. There is no universal filesystem precedence rule.

## Contract
A mapping declaration is not a security grant. An active ApplicationInstance cannot mutate its accepted mapping table in place; a change creates a new validated mapping state and may require security revalidation.

Physical DATA/SYSTEM paths remain implementation details and must not leak into Bundle mapping declarations.

## Dependencies
Consumes filesystem/path primitives and shared values. Security may consume mapping plans, but this crate must not depend upward on the security authority merely to validate permissions.

## Open
Final mapping classes, materialization strategy and complete System Image lazy-access implementation require further integration work.
