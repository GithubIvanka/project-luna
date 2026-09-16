---
name: luna-architecture
description: Apply Project Luna's accepted architecture and boundaries when designing or reviewing changes.
---
# Luna Architecture
Read current architecture and contract documents before architectural changes.
Use `docs/architecture/ACCEPTED-DECISIONS.md` and `docs/ARCHITECTURE.md` as primary context.
Preserve existing component boundaries and avoid new crates without clear responsibility.
Prefer existing Linux primitives when they satisfy the accepted design.
Do not revive rejected architecture from archives or stale code.
Check cross-component consequences before changing public contracts or ABIs.
Update current documentation when an accepted architectural decision changes.
Treat workarounds as local implementation details, not new architectural layers.
