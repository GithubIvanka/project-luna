# Project Luna — матрица состояния компонентов

**Статус:** канонический навигационный документ.  
**Источник истины:** `docs/ARCHITECTURE.md`.  
**Подробные контракты:** `docs/architecture/components/`.

| Граница | Текущее состояние | Контракт | Основная оставшаяся работа |
|---|---|---|---|
| `luna-boot.efi` | реализован development boot path; hardening продолжается | `LUNA-BOOT.md` | production handoff/fallback state/integration |
| System Image | payload собирается | `SYSTEM-IMAGE.md` + Phase 0 contract | финальная manifest/compatibility specification |
| Recovery/Factory | архитектура определена | `RECOVERY-FACTORY.md` + Phase 0 contract | полноценные среды и repair flow |
| `luna-common` | реализован | `LUNA-COMMON.md` | сохранить минимальным |
| `luna-fs` | foundation реализован | `LUNA-FS.md` | production backend integration |
| `luna-root-mapping` | foundation реализован | `LUNA-ROOT-MAPPING.md` | lazy-root и materialization integration |
| `luna-namespace` | initial Linux backend | `LUNA-NAMESPACE.md` | production isolation/security integration |
| `luna-config` | foundation реализован | `LUNA-CONFIG.md` | final persistence/serialization integration |
| `luna-security` | policy foundation | `LUNA-SECURITY.md` | enforcement, trust, confirmation IPC |
| `luna-state` | durable boundary + redb | `LUNA-STATE.md` | migrations/reconciliation |
| `luna-event` | domain boundary | `LUNA-EVENT.md` | transport/durability integration |
| `luna-bundle` | LBP1 implementation | `LUNA-BUNDLE.md` | conformance, hardening, integration |
| `luna-app-manager` | boundary/scaffold | `LUNA-APP-MANAGER.md` | install/dependency/data lifecycle |
| `luna-system-manager` | boundary/scaffold | `LUNA-SYSTEM-MANAGER.md` | durable system model |
| `luna-update-manager` | transaction foundation | `LUNA-UPDATE-MANAGER.md` | mutation/reconciliation/rollback |
| `luna-kernel-manager` | inventory/build direction | `LUNA-KERNEL-MANAGER.md` | boot/update integration |
| `luna-device-manager` | boundary/scaffold | `LUNA-DEVICE-MANAGER.md` | discovery/automount/eject |
| `luna-system-runtime` | core supervision/session orchestration | `LUNA-SYSTEM-RUNTIME.md` | production session/privilege/reconciliation |
| `UserSession` | domain lifecycle implemented | `USER-SESSION.md` | switching/restriction/logout/auth IPC |
| `luna-app-runtime` | ApplicationInstance boundary implemented | `LUNA-APP-RUNTIME.md` | namespace/security/resource integration |
| `luna-login` | greetd/Noctalia integration | `LUNA-LOGIN.md` | final Luna authentication IPC |
| `luna-cli` | thin client boundary | `LUNA-CLI.md` | command/IPC surface |
| `luna-files` | GTK4 GUI | `LUNA-FILES.md` | operations/navigation/backend integration |
| `luna-audio` | domain boundary | `LUNA-AUDIO.md` | PipeWire/WirePlumber provider |
| `luna-network` | domain boundary | `LUNA-NETWORK.md` | NetworkManager/D-Bus provider |
| `luna-bluetooth` | domain boundary | `LUNA-BLUETOOTH.md` | BlueZ/D-Bus provider |
| `luna-init` | standalone musl early userspace | `docs/ARCHITECTURE.md` | hardening and complete logical-root handoff |

## Как читать матрицу

Матрица не является самостоятельным implementation checklist. Перед изменением границы нужно читать её contract и решения, на которые он ссылается.

«Реализован» здесь означает наличие осмысленного кода на границе, а не завершённость соответствующей возможности всей ОС.
