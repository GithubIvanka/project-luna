# Project Luna — Component Status Matrix

**Status:** canonical navigation aid; detailed contracts live beside this file
**Authority:** `docs/ARCHITECTURE.md`

| Boundary | Current implementation | Contract | Main remaining work |
|---|---|---|---|
| `luna-boot.efi` | Partial / proven test boot path | `LUNA-BOOT.md` | real image/kernel selection, fallback, final handoff |
| System Image | Build payload exists | `SYSTEM-IMAGE.md` | final manifest/compatibility specification |
| Recovery/Factory | Architecture defined | `RECOVERY-FACTORY.md` | complete environments and repair flow |
| `luna-common` | Implemented | `LUNA-COMMON.md` | keep minimal |
| `luna-fs` | Implemented foundation | `LUNA-FS.md` | production backend integration |
| `luna-root-mapping` | Implemented foundation | `LUNA-ROOT-MAPPING.md` | materialization/lazy-root integration |
| `luna-namespace` | Initial Linux backend | `LUNA-NAMESPACE.md` | production isolation/security integration |
| `luna-config` | Implemented foundation | `LUNA-CONFIG.md` | final persistence/serialization integration |
| `luna-security` | Policy foundation | `LUNA-SECURITY.md` | enforcement, trust, confirmation IPC |
| `luna-state` | Durable boundary + redb | `LUNA-STATE.md` | migrations/reconciliation |
| `luna-event` | Domain boundary | `LUNA-EVENT.md` | transport/durability integration |
| `luna-bundle` | LBP1 implementation | `LUNA-BUNDLE.md` | conformance/hardening/integration |
| `luna-app-manager` | Boundary/scaffold | `LUNA-APP-MANAGER.md` | complete install/dependency/data lifecycle |
| `luna-system-manager` | Boundary/scaffold | `LUNA-SYSTEM-MANAGER.md` | durable system model |
| `luna-update-manager` | Transaction foundation | `LUNA-UPDATE-MANAGER.md` | domain mutation/reconciliation/rollback |
| `luna-kernel-manager` | Inventory/build direction | `LUNA-KERNEL-MANAGER.md` | boot/update integration |
| `luna-device-manager` | Boundary/scaffold | `LUNA-DEVICE-MANAGER.md` | real discovery/automount/eject |
| `luna-system-runtime` | Core supervision/session orchestration | `LUNA-SYSTEM-RUNTIME.md` | production session/privilege/reconciliation |
| `UserSession` | Domain lifecycle implemented | `USER-SESSION.md` | switching/restriction/logout/auth IPC |
| `luna-app-runtime` | ApplicationInstance boundary implemented | `LUNA-APP-RUNTIME.md` | real namespace/security/resource integration |
| `luna-login` | greetd/Noctalia integration | `LUNA-LOGIN.md` | final Luna authentication IPC |
| `luna-cli` | Thin client boundary | `LUNA-CLI.md` | complete command/IPC surface |
| `luna-files` | GTK4 GUI | `LUNA-FILES.md` | operations/navigation/backend integration |
| `luna-audio` | Domain boundary | `LUNA-AUDIO.md` | PipeWire/WirePlumber provider |
| `luna-network` | Domain boundary | `LUNA-NETWORK.md` | NetworkManager/D-Bus provider |
| `luna-bluetooth` | Domain boundary | `LUNA-BLUETOOTH.md` | BlueZ/D-Bus provider |

## Reading rule

This matrix is not an implementation checklist by itself. Before changing a boundary, read its component contract and the accepted decisions it references.

A row marked implemented does not mean the whole OS capability is complete; it means the architectural boundary has meaningful code.
