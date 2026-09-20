# Core

`components/core/` — основная runtime часть Luna OS.

Здесь находятся boot-adjacent и system-foundation компоненты:

- `luna-init`
- `luna-system-runtime`
- `luna-user-session`
- `luna-device-manager`
- `luna-app-runtime`
- `luna-security`
- `luna-namespace`
- `luna-root-mapping`
- `luna-fs`
- `luna-state`
- `luna-config`
- `luna-event`
- `luna-common`
- `luna-bundle`
- `luna-system-manager`
- `luna-update-manager`
- `luna-kernel-manager`

Компоненты здесь являются foundation contracts. Отдельный crate не означает отдельный daemon/process.