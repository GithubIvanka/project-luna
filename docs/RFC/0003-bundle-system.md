# RFC-0003 — Luna Bundle Format

Status: Draft

## Summary

In Luna, every installable component is a Bundle. Bundles are self-contained units with a manifest, payload, resources, and signature. They enable atomic installs, updates, rollbacks, and permission control.

## Design Goals

- Unified format for all components (apps, drivers, libs, runtimes, services).
- Self-contained and portable.
- Verifiable via cryptographic signature.
- Declarative dependencies and permissions.
- Easy rollback and isolation.

## Bundle Types

| Type | Extension | Example |
| :--- | :--- | :--- |
| Application | `.app` | `Firefox.app` |
| Driver | `.driver` | `amdgpu.driver` |
| Library | `.lib` | `gtk4.lib` |
| Runtime | `.runtime` | `python.runtime` |
| Service | `.service` | `pipewire.service` |

## Common Structure

Every Bundle is a directory with the following structure:

```text
BundleName.ext/
├── manifest.toml # Mandatory TOML manifest
├── payload/ # Executables, kernel modules, libraries
├── resources/ # Icons, translations, docs
└── signature # Cryptographic signature (Ed25519)
```

## manifest.toml Examples

### Application Bundle

```toml
type = "application"
name = "Firefox"
id = "org.mozilla.firefox"
version = "130.0.1"

runtime = "gtk4.runtime"
libs = ["nss.lib", "cairo.lib"]

permissions = [
  "network",
  "filesystem.home",
  "audio"
]

entry_point = "payload/bin/firefox"
```
### Driver Bundle

```toml
type = "driver"
name = "AMDGPU"
id = "org.luna.driver.amdgpu"
version = "6.18"

kernel_min = "6.18"
hardware_ids = [
  "pci:1002:*"
]

priority = 100
```

### Service Bundle
```toml
type = "service"
name = "PipeWire"
restart = "always"
requires = ["dbus.service"]
entry_point = "payload/usr/bin/pipewire"
```
