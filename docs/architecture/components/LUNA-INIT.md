# `luna-init`

## Назначение

`luna-init` — первый userspace-процесс нормальной Luna-загрузки и PID 1 на всём жизненном цикле системы.

## Владеет

- приёмом и проверкой `LunaBootHandoffV1` через FD 3;
- определением boot mode и выбранных identity;
- использованием выбранного System Image как immutable source;
- разрешением normal или Recovery DATA provider;
- подготовкой RAM-backed logical `/` без root pivot;
- созданием необходимых runtime resources;
- материализацией boot-critical resources;
- запуском `luna-system-runtime` как дочернего процесса;
- обязательным reap дочерних процессов как PID 1.

## Не владеет

`luna-init` не выполняет UEFI discovery, не управляет Boot Menu, не устанавливает Bundle, не принимает application authorization и не владеет долгоживущим system supervision.

## Артефакт

```text
LUNA-SYS/cores/luna-X.Y.Z.init
LUNA-SYS/cores/luna-X.Y.Z.toml
```

`.init` — ELF64 executable artifact. Manifest `luna-init` объявляет совместимые kernels; совместимость System Image с init объявляется только в manifest соответствующего System Image.

## Bootstrap

```text
Linux kernel
   ↓
luna-init (PID 1)
   ↓
FD 3 / boot context
   ↓
selected System Image + DATA
   ↓
RAM-backed logical /
   ↓
boot-critical materialization
   ↓
spawn luna-system-runtime
```

`luna-init` остаётся PID 1.

## Direct initial userspace

Kernel запускает memory-resident `.init` через существующий Linux ELF/binfmt путь.

## Ошибки

Невалидный handoff, повреждённый `.init`, недоступный обязательный ресурс System Image или невозможность подготовить runtime означают failure текущей boot attempt. Нельзя добавлять второй init, shell bootstrap или новый runtime layer.

## Статус

FD 3 и базовая жизнь PID 1 реализованы. Полный bootstrap logical root, materialization и production startup orchestration ещё в разработке.
## Persistent per-user directory ownership

During normal or Recovery logical-root materialization, PID 1 creates the
selected user's persistent home, data and config directories on DATA. It
reads UID/GID from the selected auth provider's passwd entry and assigns those
directories to that identity before launching System Runtime. This is required
for an authenticated session to write its home files and user caches; the
selected user identity remains sourced from the auth provider rather than
being hard-coded in the mount/bootstrap logic.

## Завершение системы — текущий compatibility path

PID 1 обрабатывает стандартные сигналы BusyBox init: `SIGUSR1` для halt,
`SIGUSR2` для poweroff, `SIGTERM`/`SIGINT` для reboot. Handler только сохраняет
запрос; завершение выполняется в основном цикле. PID 1 продолжает reap
усыновлённых процессов, пока работает System Runtime.

После запроса PID 1 даёт процессам две секунды на завершение через SIGTERM,
останавливает оставшиеся, вызывает sync и финальный Linux reboot syscall.
Сам syscall разрешён только в PID 1. Это минимальный путь для внешнего elogind
provider; полная координация shutdown-служб остаётся ответственностью
`luna-system-runtime` и ещё требует отдельной реализации.
