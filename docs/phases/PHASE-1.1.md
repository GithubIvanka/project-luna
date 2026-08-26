# Project Luna — Phase 1.1

## Logical Root, Path Mapping & Namespace Model

**Status:** Accepted working architecture  
**Date:** 2026-08-18

Phase 1.1 establishes the filesystem compatibility model that lets Linux software see conventional paths while Luna keeps DATA physically clean.

### Accepted model

```text
Linux kernel
    ↓
minimal RAM logical root
    ↓
System Image logical content
    ↓
controlled DATA-backed mappings
    ↓
application/user namespaces
```

`luna-root` owns logical root construction and mapping composition. Existing Linux mechanisms should be used rather than inventing an entirely new mount/init system, while responsibilities remain separated.

Mappings are file-oriented, policy-controlled and namespace-local. There is no global mapping table.

Every application receives its own mount/filesystem namespace. Its view is assembled from its bundle, required dependencies, permitted user/system resources, explicitly authorized volumes and devices.

Lookup precedence:

```text
application → user → system
```

`/etc`, `/home`, `/usr`, `/lib`, `/bin`, `/var` and similar paths are logical compatibility interfaces rather than physical requirements for recreating the Linux directory tree in DATA.

### Still open

- exact `luna-root` API/process split;
- exact mount API sequence;
- exact SquashFS lazy-loading mechanism;
- mapping-table representation;
- path-class taxonomy;
- per-path read/write policy;
- exact `/var` decomposition;
- namespace persistence mechanism;
- service-manager integration.
