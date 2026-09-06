# `luna-init`

**Статус:** accepted architecture; implementation in progress.

## Назначение

`luna-init` — standalone early-userspace bootstrap Luna. Он является первым userspace-компонентом после Linux kernel и подготавливает рабочую среду, в которой затем запускается `luna-system-runtime`.

`luna-init` не является обычным system manager и не должен превращать System Image в постоянный физический Linux root.

## Boot chain

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
luna-init
  ↓
RAM-backed logical /
  ↓
luna-system-runtime (PID 1)
  ↓
UserSession(s)
```

## Responsibilities

1. Получить boot context и параметры выбранной System Image/kernel pair.
2. Обнаружить и проверить SYSTEM/DATA.
3. Подключить SYSTEM в read-only режиме.
4. Найти и проверить `SYSTEM/images/luna-X.Y.Z.squashfs` и его manifest.
5. Подключить System Image как внутренний immutable source, а не как финальный `/`.
6. Создать RAM-backed logical root.
7. Создать runtime pseudo-filesystems и volatile paths (`/dev`, `/proc`, `/sys`, `/run`, `/tmp`).
8. Материализовать boot-critical system base в RAM.
9. Передать DATA и другие разрешённые ресурсы через контролируемые runtime mappings.
10. Запустить `luna-system-runtime`, который становится PID 1 нормального userspace.

## Root model

Рабочий `/` — это runtime root, backing store которого находится в RAM. System Image является неизменяемым источником данных.

```text
SYSTEM/images/luna-X.Y.Z.squashfs
              │
              ▼
        internal source
              │
       ┌──────┴──────┐
       │             │
   boot-critical   lazy hydration
   materialize       later
       │             │
       └──────┬──────┘
              ▼
       RAM-backed logical /
```

Luna не копирует весь System Image в RAM при старте и не использует SquashFS как долгоживущий lower/root filesystem для рабочей системы. Начальный набор определяется boot-critical contract; дополнительные immutable resources materialize по мере необходимости.

После materialization рабочий resource должен быть независим от того, остаётся ли исходный System Image подключённым. Отсоединение/удаление image допускается только после проверки, что все ещё необходимые runtime resources уже materialized либо иным образом гарантированно доступны.

## Process model

`luna-init` не создаёт отдельный application init.

```text
PID 1
luna-system-runtime
    │
    ├── UserSession
    │     └── luna-app-runtime
    │            └── ApplicationInstance → application process
    │
    └── system services
```

`luna-app-runtime` является runtime-компонентом, а не отдельным PID 1. `luna-app-init` отсутствует.

По умолчанию application process не помещается в отдельный PID namespace. Он остаётся обычным процессом системного PID namespace и получает обычный non-1 PID. Остальные isolation primitives выбираются policy/runtime profile.

## Failure behavior

Ошибка любого обязательного bootstrap шага должна останавливать нормальную загрузку и переводить систему в recovery/emergency path, а не оставлять частично сформированный logical root.

Внутренняя materialization должна быть транзакционной: уже созданные mounts/resources откатываются при ошибке до передачи управления `luna-system-runtime`.

## Current implementation gap

Текущая `components/luna-init/src/main.rs` всё ещё содержит исторический development path с BusyBox `switch_root`. Это не является принятой архитектурой и должно быть заменено RAM-root bootstrap с boot-critical materialization.

Перед production implementation необходимо зафиксировать boot-critical materialization manifest/dependency closure и механизм lazy hydration.
