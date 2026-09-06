# Project Luna — текущее состояние

**Последнее обновление:** 2026-09-06

> `docs/ARCHITECTURE.md` является архитектурным Source of Truth. Принятые решения до Phase 1.6-HZ и последующие принятые решения консолидированы там; исторические записи в `docs/decisions/` сохраняют трассируемость.

## Общее состояние

Архитектурный цикл завершён через **Phase 1.1–1.6-HZ**. Сейчас Project Luna находится в **Phase 2: runtime/boot integration, PC bring-up, desktop integration и hardening**.

## Состояние областей

| Область | Состояние |
|---|---|
| Architecture 1.1–1.6-HZ | принято и консолидировано |
| Post-1.6 architecture decisions | консолидированы в SoT и decision records |
| Foundation/domain APIs | baseline реализован |
| Runtime hierarchy | `luna-system-runtime → UserSession → luna-app-runtime → ApplicationInstance` |
| Typed runtime contract | `RuntimeKind` / `RuntimeSpec` реализованы как properties execution environment |
| Runtime profile | `RuntimeProfile::minimal()` добавлен как явный contract trusted system resources |
| Capability contract | `CapabilityName` / `CapabilityRegistry` / `CapabilityGrant` / `CapabilityProvider` добавлены; provider invocation ещё не подключён |
| Generic `luna-runtime` | явно отклонён и удалён |
| Runtime ↔ mapping ↔ Security | contract реализован; authorization предшествует namespace materialization |
| Linux namespace/materialization | development backend реализован; RAM-backed logical root/profile-driven system view и production child-creation hardening продолжаются |
| Persistent state | durable `redb` backend в `DATA/system/state` реализован |
| Update/checkpoint/rollback | durable orchestration реализована; конкретные mutation backends продолжаются |
| Bundle Format v1 | **RFC-0002 принят 2026-08-30; LBP1 проходит conformance/security hardening** |
| `luna-system-runtime` | real child supervision и UserSession lifecycle ownership реализованы |
| `luna-app-runtime` | ApplicationInstance lifecycle, ApplicationPlan и typed execution boundary реализованы |
| `luna-init` | native musl early-userspace с RAM-backed tmpfs root, explicit bootstrap subset, отдельными runtime pseudo-filesystems и финальным `chroot` в `luna-system-runtime` реализован; lazy hydration и dependency-closure ещё продолжаются |
| `luna-boot.efi` | GUI splash; dynamic image/kernel discovery; manifest validation; compatible kernel selection; soft fallback; ordered Boot Menu |
| Recovery / Factory boot | discovery и target execution реализованы; repair tooling/UX ещё не завершены |
| External/USB boot | UEFI `EFI/BOOT/BOOTX64.EFI` chainload development backend реализован |
| x86_64 PC image | воспроизводимый GPT/UEFI development image builder реализован |
| Graphical desktop | login boundary существует; финальная niri + Noctalia + seat/device integration продолжается |

## Нормальная загрузка

```text
Power
 ↓
UEFI
 ↓
luna-boot.efi
 ↓
Luna GUI boot splash
 ↓
Linux kernel
 ↓
luna-init
 ↓
internal immutable System Image source
 ↓
RAM-backed tmpfs logical /
 ↓
luna-system-runtime (PID 1)
 ↓
UserSession
 ↓
GUI login
 ↓
authentication
 ↓
Active UserSession
 ↓
Wayland
 ↓
niri
 ↓
Noctalia Shell
```

Обычный вход не использует TTY, console shell или `luna-session`.

`B` открывает исключительное текстовое Boot Menu.

## Runtime

Владение:

```text
luna-system-runtime (PID 1)
├── UserSession
│   └── luna-app-runtime
│       └── ApplicationInstance
```

Execution pipeline:

```text
Bundle
  ↓
ApplicationPlan
  ↓ validate
luna-security
  ↓ Allow
AuthorizedApplicationPlan
  ↓
ApplicationLaunchContext + RuntimeProfile
  ↓
luna-namespace
  ↓
process execution
  ↓
ApplicationInstance
```

`ApplicationInstance` является representation конкретного execution lifecycle, а не security policy boundary.

`RuntimeKind` — только тип execution environment; generic `luna-runtime` отсутствует.

## Security model

Filesystem access и named capabilities разделены.

```text
Filesystem:
  read / write / execute — explicit
  empty access            — no access

Capabilities:
  namespaced identity
  explicit registry entry
  explicit authorization
  typed CapabilityGrant
  no implicit inheritance
```

`luna-security` решает, разрешён ли запрос. `luna-namespace` и provider backends должны применять уже авторизованный результат.

### Текущее состояние namespace

Production materialization уже использует RAM-backed logical root и profile-selected trusted resources вместо полного System Image OverlayFS lower layer. Открыты hardening/integration задачи для privileged Linux tests, filtered `/dev`, child-creation primitive и полной lazy hydration.

## System initialization

```text
Linux kernel
    ↓
initramfs
    ↓
/init = luna-init
    ↓
SYSTEM + DATA + selected System Image source
    ↓
/tmpfs logical /
    ↓
chroot
    ↓
/sbin/luna-system-runtime = system PID 1
```

Классический `switch_root` к SquashFS root больше не используется целевым `luna-init` path.

## Process/PID model

```text
system PID namespace
└── PID 1 luna-system-runtime
    ├── system services
    ├── UserSession
    │   └── luna-app-runtime / ApplicationInstance
    └── application processes
```

`luna-app-init` отсутствует. PID namespace не требуется по умолчанию для application isolation. Приложение запускается непосредственно как `ApplicationInstance` и получает обычный non-1 system PID.

## Ближайшие технические приоритеты

1. Зафиксировать boot-critical materialization manifest/dependency closure и реализовать безопасную материализацию из внутреннего System Image source.
2. Реализовать lazy System Image hydration так, чтобы materialized resources не зависели от lifetime image.
3. Завершить physical symlink/containment hardening для mapping roots и staging paths.
4. Подключить capability providers через IPC, сохранив security decision исключительно в `luna-security`.
5. Довести PC image до воспроизводимой полной загрузки в QEMU/OVMF и проверить на реальном UEFI hardware.
6. Завершить graphical login + niri + Noctalia integration.
7. Завершить resource limits/cgroups, restart policy и lifecycle reconciliation.
8. Завершить durable boot/update success/failure state.
9. Завершить LBP1 conformance и Ed25519 trust binding.
10. Реализовать filtered `/dev`, device/volume integration и `.lbp` install → ApplicationInstance launch/recovery loop.
11. Заменить prototype `pre_exec` namespace setup production-safe child-creation primitive.

## Phase 0 documentation

Канонические документы bootstrap:

```text
docs/architecture/components/LUNA-INIT.md
docs/contracts/LUNA-INIT-CONTRACT.md
docs/decisions/0008-pid-namespace-and-ram-hydration.md
```

Они описывают принятую модель: `luna-init` строит RAM-backed logical `/`, `luna-system-runtime` является PID 1, а отдельного `luna-app-init` нет.
