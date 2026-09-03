# `luna-device-manager`

**Status:** accepted boundary; integration incomplete

## Purpose
Discover devices and manage volume lifecycle, including Luna's automatic external-volume behavior.

## Owns
- device discovery/lifecycle;
- filesystem/volume discovery;
- automount orchestration;
- friendly volume identity exposed to clients;
- safe unmount/eject lifecycle;
- managed volume state under `DATA/system/volumes`.

## Does not own
Raw kernel driver implementation, filesystem primitives, application permissions, file-manager UI or application execution.

## External-media contract

```text
USB inserted
 ↓
device detected
 ↓
filesystem detected
 ↓
automount
 ↓
volume appears to file manager
```

No manual `/dev/...` or `/mnt/...` workflow is required for normal use.

Connecting media must not silently execute an application. Autorun is disabled or confirmation-based according to policy.

## Dependencies
`luna-fs`, `luna-security`, device/kernel facilities, and event/state contracts.

## Open
Concrete device backend, filesystem probing, automount implementation and network-volume extension remain.
