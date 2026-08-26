# Project Luna — Phase 1.2

## Runtime, Sessions, Users, Recovery & Application Lifecycle

**Status:** Accepted working architecture through AA–AR  
**Date:** 2026-08-18

### Boot and recovery

```text
minimal RAM root
    ↓
System Image
    ↓
DATA
    ↓
normal users
```

If DATA is unavailable, boot into Recovery with a temporary virtual user whose state lives in RAM. If the System Image cannot start, use the immutable Factory System Image + Factory Kernel pair.

### Users

Multiple users may be active simultaneously. Session behavior is configurable per user: continue, restricted, or terminate; default is restricted.

### Applications

Applications use application-specific namespaces with file-level mappings, visibility and permissions. Bundles are immutable; mutable application data and configuration are user/application-owned DATA state.

### Runtime ownership

- App Manager: install, launch, update, remove, application-data lifecycle.
- Runtime/Namespace layer: application process and namespace lifecycle.
- `luna-root`: logical root and mapping composition.
- Updater/system management: SYSTEM writes and system-image/kernel lifecycle.
- Recovery/Factory: repair and fallback paths.

### Checkpoints

Btrfs snapshots provide configurable checkpoints/rollback for selected DATA state. They are not runtime session switching and not long-term backups.

### Still open

- exact namespace API;
- restricted-session implementation;
- memory-pressure eviction algorithm;
- dependency discovery/download protocol;
- permission policy language/enforcement;
- diagnostic/emergency state;
- checkpoint scope and retention;
- updater transaction semantics across user switches.
