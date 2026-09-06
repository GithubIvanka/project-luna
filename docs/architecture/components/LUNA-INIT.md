# `luna-init`

**Статус:** accepted architecture; RAM-root bootstrap implementation in progress.

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
6. Создать отдельный RAM-backed tmpfs logical root.
7. Создать runtime pseudo-filesystems и volatile paths (`/dev`, `/proc`, `/sys`, `/run`, `/tmp`).
8. Материализовать boot-critical system base в RAM.
9. Передать DATA и другие разрешённые ресурсы через контролируемые runtime mappings.
10. Запустить `luna-system-runtime`, который становится PID 1 нормального userspace.

## Root model

Рабочий `/` — это отдельный tmpfs runtime root, backing store которого находится в RAM. System Image является неизменяемым источником данных.

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

Текущая реализация уже создаёт tmpfs root, materializes явный bootstrap subset и отсоединяет внутренний System Image source до передачи управления runtime. Полная manifest-driven dependency closure и lazy hydration ещё не завершены.

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

Внутренняя materialization должна быть транзакционной: уже созданные mounts/resources откатываются при ошибке до передачи управления `luna-system-runtime`. Полный rollback implementation для всех промежуточных ошибок остаётся hardening item.

## Current implementation status

`components/luna-init/src/main.rs` больше не использует классический `switch_root`.

Реализованы:

- отдельный tmpfs для logical `/`;
- независимое подключение DATA;
- внутреннее read-only подключение System Image;
- явный bootstrap subset вместо полной копии image;
- materialization через `cp -a` с сохранением обычной семантики файлов и symlink;
- runtime-generated `/dev`, `/proc`, `/sys`, `/run` и `/tmp`;
- отсоединение System Image/SYSTEM source перед control transfer;
- `chroot` в RAM-backed root с последующим запуском `/sbin/init = luna-system-runtime`.

Открыто:

- manifest-driven bootstrap manifest и dependency closure;
- защищённая материализация всех поддерживаемых filesystem object types;
- lazy hydration service/protocol;
- финальная политика `/dev`;
- privileged QEMU/UEFI end-to-end tests;
- image-retirement checks, связанные с фактическим runtime materialization state.
