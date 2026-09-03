# Project Luna — Decision to Component Map

**Status:** navigation/consistency aid
**Authority:** `docs/ARCHITECTURE.md` and accepted decisions

This file answers: “which decision should I read before touching this boundary?” It does not create new decisions.

| Area | Accepted record(s) | Component contract |
|---|---|---|
| Physical DATA layout | ADR-0001 | `DISK-LAYOUT.md` |
| Boot / Recovery / Factory | ADR-0002 + current boot decisions | `LUNA-BOOT.md`, `RECOVERY-FACTORY.md` |
| Logical root | ADR-0003 | `LUNA-ROOT-MAPPING.md` |
| Namespace / mappings | ADR-0004 + Phase 1.6 contracts | `LUNA-ROOT-MAPPING.md`, `LUNA-NAMESPACE.md` |
| Application lifecycle | ADR-0005 + runtime contract | `LUNA-APP-MANAGER.md`, `LUNA-APP-RUNTIME.md` |
| User sessions/checkpoints | ADR-0006 + current graphical-session decisions | `USER-SESSION.md`, `LUNA-SYSTEM-RUNTIME.md` |
| Bundle Format v1 | RFC-0002 + ADR-0007 | `LUNA-BUNDLE.md` |
| Graphical boot/login | 2026-09-01 graphical boot/session decisions | `LUNA-LOGIN.md`, `USER-SESSION.md`, `LUNA-SYSTEM-RUNTIME.md` |
| Runtime taxonomy | 2026-09-01 runtime contract | `LUNA-APP-RUNTIME.md`, `LUNA-SYSTEM-RUNTIME.md` |
| Git/branch workflow | 2026-09-01 Git workflow decision | `docs/development/` governance |

## Important historical distinction

The repository contains both chronological ADRs and newer date-named decisions. A date-named decision can refine the current implementation without changing an unrelated older decision, but any explicit supersession must be stated in the newer record.

## Before changing a component

Read the component contract first, then the decision records listed here. If the decision records conflict, do not choose silently: identify the conflict and resolve it through an explicit decision/documentation change.
