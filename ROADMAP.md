# Project Luna — дорожная карта

Архитектурным источником истины является [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Эта дорожная карта описывает порядок реализации и зависимости, но не устанавливает сроки.

## Текущее положение

Архитектурные фазы **1.1–1.6-HZ** приняты и консолидированы. RFC-0002 Bundle Format v1 принят 2026-08-30. Проект находится в **Phase 2: интеграция runtime/boot, PC bring-up, desktop integration и hardening**.

## Уже реализованная основа

```text
Architecture / SoT                ← ЗАВЕРШЕНО
Repository / Cargo audit          ← ЗАВЕРШЕНО
Domain + manager API baseline     ← ЗАВЕРШЕНО
Logical mapping backend           ← РЕАЛИЗОВАНО
Linux namespace backend           ← РЕАЛИЗОВАНО
Persistent redb state             ← РЕАЛИЗОВАНО
Update/checkpoint engine          ← РЕАЛИЗОВАНО
RFC-0002 / LBP1                   ← ПРИНЯТО / HARDENING
System runtime supervisor         ← РЕАЛИЗОВАНО
UserSession graphical lifecycle   ← РЕАЛИЗОВАНО
Typed runtime contract            ← РЕАЛИЗОВАНО КАК VALUE TYPE
Runtime ↔ mapping ↔ Security      ← КОНТРАКТ РЕАЛИЗОВАН
QEMU userspace bring-up           ← РЕАЛИЗОВАН РАЗРАБОТЧЕСКИЙ ПУТЬ
x86_64 PC image builder            ← РЕАЛИЗОВАН РАЗРАБОТЧЕСКИЙ ПУТЬ
Guarded PC installer              ← РЕАЛИЗОВАН РАЗРАБОТЧЕСКИЙ ПУТЬ
PC image CI workflow              ← РЕАЛИЗОВАНО
Graphical boot splash             ← РЕАЛИЗОВАН РАЗРАБОТЧЕСКИЙ ПУТЬ
System Image discovery            ← РЕАЛИЗОВАН РАЗРАБОТЧЕСКИЙ ПУТЬ
Compatible kernel selection       ← РЕАЛИЗОВАН РАЗРАБОТЧЕСКИЙ ПУТЬ
Boot Menu full action set         ← РЕАЛИЗОВАН РАЗРАБОТЧЕСКИЙ ПУТЬ
USB/External UEFI chainload       ← РЕАЛИЗОВАН РАЗРАБОТЧЕСКИЙ ПУТЬ
```

## Последовательность Phase 2

### 1. Надёжная загрузка графического PC-образа

Основной артефакт:

```text
dist/luna-pc.img
```

В нём присутствуют EFI, SYSTEM и DATA. SYSTEM содержит versioned SquashFS System Image, manifest, initramfs и kernel. DATA хранит постоянное состояние Luna.

Следующие действия:

- проверить полный boot path в QEMU/OVMF;
- проверить реальный UEFI hardware;
- сохранить label-based discovery SYSTEM/DATA;
- завершить persistent boot-success state.

### 2. Materialization runtime

Типизированный runtime:

```text
RuntimeKind::Luna   → native Luna userspace / musl
RuntimeKind::Glibc  → approved glibc compatibility runtime
RuntimeKind::Bundle → Bundle-private runtime
```

`RuntimeKind` — свойство `ApplicationInstance`, а не менеджер или уровень иерархии.

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance { RuntimeSpec }
```

### 3. Security и device boundary

Завершить:

- fine-grained mapping authorization;
- filtered `/dev` population;
- secure physical-path и symlink validation;
- resource enforcement before execution;
- device authorization и volume integration.

### 4. Полноценный графический System Image

Цепочка:

```text
UEFI
  ↓
luna-boot.efi
  ↓
GUI boot splash
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

Нет штатного TTY login или shell fallback. `B` открывает исключительное Boot Menu:

```text
1. Continue to Luna
2. Verbose Boot
3. System Image selection
4. Recovery Environment
5. Factory Environment
6. Boot from USB / External Device
```

### 5. Boot discovery и recovery

`luna-boot.efi` уже работает с реальным деревом SYSTEM:

```text
SYSTEM/images/*.squashfs
        +
SYSTEM/images/*.toml
        ↓
manifest validation
        ↓
SYSTEM/kernels/<version>/bzImage
        ↓
compatible kernel filtering
        ↓
version ordering
        ↓
BootTarget catalog
```

Recovery и Factory являются специальными System Image roles. External Boot — отдельный UEFI chainload path.

### 6. Bundle install → application execution

Завершить development loop:

```text
.lbp
 ↓
verify
 ↓
install into DATA
 ↓
ApplicationPlan
 ↓
MappingPlan
 ↓
Security
 ↓
Namespace
 ↓
ApplicationInstance
 ↓
process supervision
```

`.lbp` остаётся отдельным Bundle format; System Image остаётся SquashFS.

### 7. Durable update / boot state

Связать `luna-system-manager`, `luna-kernel-manager`, `luna-app-manager` и `luna-update-manager` с конкретными mutation backends, сохранив независимость обновлений System Image и kernel и revision-checked durable state.

### 8. Production hardening

- полная LBP1 conformance и Ed25519 trust binding;
- итоговый IPC/event transport;
- resource controls;
- production-safe child creation вместо сложного `pre_exec` namespace setup;
- Secure Boot и release-image signing;
- recovery и interrupted-update validation.

## Phase 0 — архитектурные контракты

Перед следующим крупным implementation cycle нужно отдельно рассмотреть пять draft contracts:

```text
docs/contracts/SYSTEM-IMAGE-CONTRACT.md
docs/contracts/KERNEL-CONTRACT.md
docs/contracts/BOOT-STATE-CONTRACT.md
docs/contracts/BOOT-HANDOFF-CONTRACT.md
docs/contracts/FAILURE-RECOVERY-CONTRACT.md
```

Они находятся в `develop` как черновики и не меняют SoT до явного принятия.

## Правила Git

`main` — каноническая интеграционная ветка. Обычная работа ведётся в короткоживущей ветке от актуального `main` с PR в `main`. Текущая рабочая ветка документационной консолидации — `develop`.

## Неподлежащие пересмотру ограничения

- System Image = непосредственно SquashFS.
- `.lbp` = Bundle transport/archive format.
- SYSTEM immutable/versioned; DATA mutable.
- `luna-security` — центральная policy authority.
- `luna-root-mapping` — mapping layer.
- `luna-namespace` — namespace/materialization layer.
- `luna-system-runtime` — единственный system-wide supervisor.
- `UserSession` — объединённая user/session entity.
- TTY/serial — только development, diagnostics или recovery.
- Нормальный boot — GUI splash → graphical login → Wayland → niri → Noctalia.
- Boot Menu открывается только по явному запросу `B` и имеет фиксированный порядок Continue, Verbose, Image, Recovery, Factory, External/USB.
- `RuntimeKind` — только execution-environment value; generic `luna-runtime` отсутствует.
- Принятые решения находятся в `docs/decisions/` и консолидируются в SoT.
