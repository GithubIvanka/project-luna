# Состояние загрузки и попытки запуска

## Два разных вида состояния

Luna разделяет постоянный контекст целей загрузки, временный marker между перезагрузками и подробное состояние текущей попытки.

```text
LUNA-SYS/config/boot-state.toml
        │
        ├── current
        ├── fallback
        ├── recovery
        └── factory

UEFI NVRAM
        └── LunaBootAttempt

RAM
        └── BootAttemptProgress
```

## Постоянный boot state

`LUNA-SYS/config/boot-state.toml` хранит долгоживший boot context и атомарные targets. Каждый target содержит полную комбинацию:

```text
System Image + luna-init + kernel
```

Recovery дополнительно содержит Recovery DATA Image:

```text
System Image + luna-init + kernel + Recovery DATA Image
```

Роли:

```text
current
fallback
recovery
factory
```

Этот файл не является журналом каждой загрузки.

## Кто изменяет persistent state

Обычный старт системы не переписывает targets. Изменения происходят только при значимых переходах: обновлении системы, `luna-init`, kernel, смене/подтверждении target, подтверждённом failure, fallback или переходе в Factory/Recovery.

Изменение durable targets принадлежит `luna-update-manager`; `luna-boot.efi` их выбирает и читает, но не переписывает как часть обычной загрузки.

## BootAttemptProgress в RAM

Подробный progress текущей попытки нужен для диагностики и принятия решений о fallback во время этого запуска. Он хранится только в RAM:

```text
BootAttemptProgress
├── attempt_id
└── stage
```

Стадии монотонны:

```text
BootloaderLoaded
BootloaderCompleted
KernelHandoff
KernelStarted
InitStarted
InitReady
SystemRuntimeStarted
Success
```

Это не persistent history.

## NVRAM marker

`LunaBootAttempt` — отдельный минимальный marker в UEFI NVRAM. Он нужен, чтобы следующая загрузка могла определить, достигла ли предыдущая попытка семантического `SUCCESS`.

Текущий формат:

```text
Variable: LunaBootAttempt
State: in_progress
```

Минимальные поля:

```text
magic
format
status
attempt_id
checksum
```

## Запись marker

Для каждой новой попытки marker записывается один раз после подготовки target, kernel, `luna-init` и Luna Handoff и непосредственно перед `ExitBootServices`.

```text
prepare target
   ↓
prepare kernel
   ↓
prepare luna-init
   ↓
build Luna Handoff
   ↓
write LunaBootAttempt = in_progress
   ↓
ExitBootServices
```

Подробные стадии между этими точками не записываются в NVRAM.

## Следующая загрузка

`luna-boot.efi` читает marker из NVRAM.

```text
marker отсутствует
    ↓
предыдущая попытка достигла SUCCESS
```

```text
marker = in_progress
    ↓
предыдущая попытка не достигла SUCCESS
```

Сам marker не определяет причину. Это может быть kernel panic, reset, потеря питания или отказ до подтверждения успеха.

## Success

`luna-system-runtime` выполняет финальную semantic success confirmation после инициализации runtime и system state. После подтверждения он удаляет `LunaBootAttempt` через `efivarfs`.

После `ExitBootServices` `luna-boot.efi` больше не управляет marker.

Нормальный budget persistent BootAttempt state:

```text
start attempt → one NVRAM write
success       → one NVRAM clear
```

Очистка marker идемпотентна.

## Handoff

В Luna Handoff передаётся контекст предыдущей попытки:

```text
previous_attempt_failed
previous_attempt_id
fallback_depth
failure_code
```

Это context для текущего запуска, а не журнал всех стадий.

## Fallback

Если уже загруженный kernel остаётся работоспособным, failure System Image или раннего userspace может привести к soft fallback без reboot. Кандидат должен заново разрешить совместимую цепочку `image → init → kernel`, используя уже загруженный kernel.

Kernel panic является reboot-level failure. При следующем старте выбирается предыдущий совместимый kernel/target согласно durable state.