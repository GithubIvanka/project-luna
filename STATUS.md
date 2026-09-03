# Project Luna — текущее состояние

**Последнее обновление:** 2026-09-03

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
| Generic `luna-runtime` | явно отклонён и удалён |
| Runtime ↔ mapping ↔ Security | contract реализован; authorization предшествует namespace materialization |
| Linux namespace/materialization | development backend реализован; production child-creation hardening продолжается |
| Persistent state | durable `redb` backend в `DATA/system/state` реализован |
| Update/checkpoint/rollback | durable orchestration реализована; конкретные mutation backends продолжаются |
| Bundle Format v1 | **RFC-0002 принят 2026-08-30; LBP1 проходит conformance/security hardening** |
| `luna-system-runtime` | real child supervision и UserSession lifecycle ownership реализованы |
| `luna-app-runtime` | ApplicationInstance lifecycle и execution setup реализованы |
| `luna-init` | native musl early-userspace реализован и упаковывается как `/init` |
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
System Image + DATA
 ↓
luna-system-runtime
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

`B` открывает исключительное текстовое Boot Menu:

```text
1. Continue to Luna
2. Verbose Boot
3. System Image selection
4. Recovery Environment
5. Factory Environment
6. Boot from USB / External Device
```

## Обнаружение boot targets

`luna-boot.efi` использует реальное содержимое SYSTEM:

```text
SYSTEM/images/*.squashfs + adjacent *.toml
              ↓
       manifest validation
              ↓
SYSTEM/kernels/<version>/bzImage
              ↓
   compatible kernel filter
              ↓
      version ordering
              ↓
       BootTarget catalog
```

Recovery и Factory являются специальными ролями System Image и не входят в обычный список выбора. External boot — отдельный UEFI chainload path.

## Runtime

Владение:

```text
luna-system-runtime
├── UserSession
│   └── luna-app-runtime
│       └── ApplicationInstance
```

Execution pipeline:

```text
ApplicationPlan
    ↓
MappingPlan
    ↓
luna-security
    ↓
luna-namespace
    ↓
process execution
```

`RuntimeKind` — только тип execution environment; generic `luna-runtime` отсутствует.

## System initialization

```text
Linux kernel
    ↓
initramfs
    ↓
/init = luna-init
    ↓
prepare SYSTEM + DATA + selected System Image
    ↓
switch_root
    ↓
/sbin/init = luna-system-runtime
```

## Ближайшие технические приоритеты

1. Довести PC image до воспроизводимой полной загрузки в QEMU/OVMF и проверить на реальном UEFI hardware.
2. Завершить graphical login + niri + Noctalia integration.
3. Расширить `luna-app-runtime` runtime-specific loader/library mapping без нарушения security/mapping boundaries.
4. Закончить fine-grained security и filtered `/dev` population.
5. Завершить durable boot/update success/failure state.
6. Завершить LBP1 conformance и Ed25519 trust binding.
7. Реализовать IPC/event transport, resource enforcement и device/volume integration.
8. Завершить `.lbp` install → ApplicationInstance launch/recovery loop.
9. Заменить prototype `pre_exec` namespace setup production-safe child-creation primitive.

## Phase 0 documentation

Черновики новых архитектурных контрактов находятся в `docs/contracts/`:

```text
SYSTEM-IMAGE-CONTRACT.md
KERNEL-CONTRACT.md
BOOT-STATE-CONTRACT.md
BOOT-HANDOFF-CONTRACT.md
FAILURE-RECOVERY-CONTRACT.md
```

Они уточняют существующие архитектурные границы и не считаются принятыми до отдельного решения.