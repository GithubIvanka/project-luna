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

Нормальная архитектура не использует production initramfs, `switch_root`, `pivot_root`, временный `/init` или второй `/sbin/init`. Kernel запускает memory-resident `.init` через существующий Linux ELF/binfmt путь.

## Ошибки

Невалидный handoff, повреждённый `.init`, недоступный обязательный ресурс System Image или невозможность подготовить runtime означают failure текущей boot attempt. Нельзя добавлять второй init, shell bootstrap или новый runtime layer.

## Статус

FD 3 и базовая жизнь PID 1 реализованы. Полный bootstrap logical root, materialization и production startup orchestration ещё в разработке.