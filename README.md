# Project Luna

**Design first. Code second.**

> Современная неизменяемая операционная система поверх Linux kernel.

Project Luna — открытый проект ОС с небольшой неизменяемой системной основой, предсказуемой архитектурой, самостоятельными приложениями и чистой пользовательской файловой моделью.

## Текущее состояние

Архитектурный цикл завершён до **Phase 1.6-HZ**, после чего проект перешёл в **Phase 2: интеграция runtime/boot, PC bring-up, desktop integration и hardening**.

Текущий Source of Truth — [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Принятые решения после Phase 1.6 консолидированы там и сохраняются в decision records.

Уже реализованы важные части:

- Linux namespace/materialization primitives в `luna-namespace`;
- durable system state на `redb` в `luna-state`;
- checkpointed update/rollback orchestration в `luna-update-manager`;
- RFC-0002/LBP1 codec в `luna-bundle`;
- supervision реальных Linux child processes и UserSession lifecycle в `luna-system-runtime`;
- lifecycle `ApplicationInstance` в `luna-app-runtime`;
- standalone musl early userspace `luna-init`;
- dynamic System Image/kernel discovery, manifest validation, compatible-kernel selection, soft fallback и ordered Boot Menu в `luna-boot.efi`;
- воспроизводимый x86_64 UEFI/GPT development image с QEMU/OVMF bring-up path;
- graphical payload с niri/Noctalia, login, Ghostty, fish, Yazi, audio, network, Bluetooth и removable-media infrastructure.

Hardware seat/input/GPU и часть production integration ещё находятся в разработке.

## Архитектурная модель

```text
UEFI
  ↓
luna-boot.efi
  ↓
SYSTEM
  ├── versioned System Images (direct SquashFS)
  └── versioned kernels
  ↓
luna-init
  ↓
logical Linux root + DATA
  ↓
luna-system-runtime
  ├── UserSession A
  │   └── luna-app-runtime
  │       └── ApplicationInstance(s)
  └── UserSession B
      └── luna-app-runtime
          └── ApplicationInstance(s)
```

Физическая модель: **EFI / SYSTEM / DATA / SWAP**.

System Image — непосредственно `luna-X.Y.Z.squashfs`. DATA содержит изменяемое системное, пользовательское, application и cache состояние.

## Запуск приложения

Иерархия владения и security pipeline разделены:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

Путь запуска:

```text
Bundle declaration
    ↓
ApplicationPlan
    ↓
MappingPlan
    ↓
luna-security
    ↓
luna-namespace materialization
    ↓
process execution
```

`RuntimeKind`/`RuntimeSpec` являются типизированными свойствами execution environment. Отдельного generic `luna-runtime` компонента нет.

## Boot и пользовательский интерфейс

Нормальная загрузка графическая и тихая:

```text
Power
 ↓
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
Wayland
 ↓
niri
 ↓
Noctalia Shell
```

Обычный пользовательский вход не использует TTY. Клавиша `B` открывает исключительное текстовое Boot Menu:

```text
1. Continue to Luna
2. Verbose Boot
3. System Image selection
4. Recovery Environment
5. Factory Environment
6. Boot from USB / External Device
```

## Bundles

Приложения используют Luna Bundle Format:

```text
application.lbp
    ↓
RFC-0002 / LBP1
```

`.lbp` — транспортное/archive представление Bundle. Это не System Image.

## Разработка

Ключевой план находится в [`docs/architecture/DEVELOPMENT-ROADMAP.md`](docs/architecture/DEVELOPMENT-ROADMAP.md).

Перед реализацией Phase 0 используются черновики контрактов в [`docs/contracts/`](docs/contracts/).

PC image builder:

```bash
tools/build-pc-image.sh
```

Подробности: [`docs/development/PC-BUILD.md`](docs/development/PC-BUILD.md).

## Правило проекта

`docs/ARCHITECTURE.md` — главный и текущий архитектурный Source of Truth. Реализация не должна молча менять принятые решения. При обнаружении конфликта сначала меняется архитектурный документ/решение, затем код.