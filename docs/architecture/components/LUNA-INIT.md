# `luna-init`

**Статус:** accepted architecture; RAM-root bootstrap implementation in progress.

## Назначение

`luna-init` — standalone early-userspace bootstrap Luna. Он является первым userspace-компонентом после Linux kernel и подготавливает рабочую среду, в которой затем запускается `luna-system-runtime`.

`luna-init` не является обычным system manager и не превращает System Image в постоянный физический Linux root.

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
3. Создать RAM-backed logical root.
4. Подключить SYSTEM read-only и выбранный System Image как скрытый immutable source.
5. Подключить DATA непосредственно в будущий logical `/data`.
6. Создать runtime pseudo-filesystems и volatile paths (`/dev`, `/proc`, `/sys`, `/run`, `/tmp`).
7. Материализовать boot-critical system base в RAM.
8. Передать контроль в `luna-system-runtime`, который становится PID 1 нормального userspace.

## Root model

Рабочий `/` — это отдельный tmpfs runtime root, backing store которого находится в RAM. System Image является неизменяемым источником данных, а DATA — независимым persistent storage.

```text
physical SYSTEM ──┐
                  ├── immutable source ──→ RAM-backed logical /
physical DATA ────┘                          └→ logical /data
```

Нет отдельного `/run/luna-system` или `/run/luna-image` слоя. `/run` существует как обычный volatile runtime path и не используется для хранения физических source mounts.

SYSTEM и выбранный System Image подключаются вне будущего logical root. Перед `pivot_root` `luna-init` передаёт trusted directory FDs системному runtime, затем detaches старое initramfs дерево. Поэтому physical SYSTEM/source mounts не становятся pathname-доступными из пользовательского `/`.

Luna не копирует весь System Image в RAM при старте. Начальный набор определяется boot-critical contract; дополнительные immutable resources materialize по мере необходимости.

## SYSTEM security boundary

SYSTEM — внутреннее хранилище ОС.

```text
ordinary user / application → no access
normal runtime             → controlled read-only source access
luna-updater               → sole writer
```

Boot-time mount SYSTEM выполняется read-only. Возможность записи принадлежит отдельному privileged update path и не является частью обычного application filesystem contract.

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

Ошибка обязательного bootstrap шага останавливает нормальную загрузку и переводит систему в recovery/emergency path.

Materialization и root transition должны быть транзакционными: частично созданный logical root не должен становиться рабочим root. Полный rollback implementation остаётся hardening item.

## Current implementation status

`components/luna-init/src/main.rs` реализует прямую RAM-root модель:

- отдельный tmpfs становится будущим logical `/` через `pivot_root`;
- DATA монтируется непосредственно в RAM root как logical `/data`;
- SYSTEM монтируется read-only только как физический внутренний source;
- выбранный SquashFS монтируется read-only вне будущего logical root;
- bootstrap subset materializes в RAM через `cp -a`;
- trusted source FDs передаются в `luna-system-runtime`;
- старое initramfs дерево detaches после root transition;
- `/proc`, `/sys`, `/dev`, `/run` и `/tmp` создаются как runtime state;
- `/sbin/init` запускается уже из нового logical `/`.

Открыто:

- manifest-driven bootstrap manifest и dependency closure;
- защищённая материализация всех поддерживаемых filesystem object types;
- lazy hydration service/protocol через trusted source boundary;
- enforcement SYSTEM updater-only write authority;
- финальная политика `/dev`;
- privileged QEMU/UEFI end-to-end tests;
- image-retirement checks, связанные с фактическим runtime materialization state.
