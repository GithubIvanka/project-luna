# Контракт Boot Attempt

**Статус:** принят
**Область:** `luna-boot.efi` → Linux kernel → `luna-init` → `luna-system-runtime`

## Назначение

`BootAttempt` описывает одну конкретную попытку загрузки — от запуска `luna-boot.efi` до подтверждённого `SUCCESS` или незавершённой попытки.

Подробный progress находится только в RAM. Persistent storage содержит минимальный marker, необходимый для обнаружения незавершённой загрузки после перезагрузки.

## Progress в RAM

```text
BootAttemptProgress
├── attempt_id
└── stage
```

Допустимые монотонные стадии:

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

Стадия назад не переходит. Этот progress не является историческим журналом.

После `ExitBootServices` подробный progress продолжает жить в выделенном RAM-объекте `LunaBootProgress`, переданном через Linux `setup_data` рядом с `LunaBootHandoffV1`. Он использует тот же `attempt_id` и позволяет kernel/Rust/userspace отмечать стадии одной и той же попытки без дополнительных NVRAM-записей.

```text
KernelStarted
InitStarted
InitReady
SystemRuntimeStarted
Success
```

Ошибку раннего kernel/userspace этапа `LunaBootProgress` может пометить как `Failed` с числовым `failure_code`. Это диагностическое RAM-состояние, а не источник durable fallback policy.

## Persistent marker

Marker хранится в UEFI NVRAM, отдельно от `LUNA-SYS/config/boot-state.toml`.

```text
Variable name: LunaBootAttempt
Vendor GUID: `9f6c5d8a-5f3b-4e24-8a3c-1d3f6e2b7c91`
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

Размер marker фиксированный и мал.

## Запись

Для каждой новой попытки `luna-boot.efi` делает одну persistent-запись после подготовки всех необходимых boot objects:

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

Промежуточные стадии не записываются в NVRAM.

Если запись marker невозможна, boot не должен продолжаться так, будто crash detection гарантирован.

## Обнаружение

При следующем запуске `luna-boot.efi` читает marker.

```text
marker отсутствует → предыдущая попытка достигла SUCCESS
marker in_progress  → предыдущая попытка не достигла SUCCESS
```

Причина не определяется marker. Возможны kernel panic, reset, потеря питания или failure до userspace success.

## Attempt ID

Каждая попытка имеет собственный `attempt_id`.

Если предыдущий marker существует, новый ID получается последовательным увеличением предыдущего. Если marker отсутствует, ID детерминированно создаётся из параметров подготовленного запуска через BLAKE3 и не может быть нулевым.

ID передаётся через Luna Handoff как identity текущего запуска.

## Успех

Запуск `luna-init` сам по себе не означает успех. Финальную semantic success confirmation выполняет `luna-system-runtime` после успешной инициализации runtime и system state.

После подтверждения:

```text
confirm SUCCESS
      ↓
remove LunaBootAttempt via efivarfs
```

После `ExitBootServices` `luna-boot.efi` больше не управляет marker. Очистка идемпотентна.

## Отказы

После `ExitBootServices` bootloader не изменяет UEFI state. При kernel panic `in_progress` остаётся и обнаруживается при следующем запуске.

## Связь с Boot State

`BootAttempt` отвечает только за конкретную попытку и crash detection. `boot-state.toml` хранит долгоживший boot context и atomic targets.

В Handoff передаётся контекст предыдущей попытки:

```text
previous_attempt_failed
previous_attempt_id
fallback_depth
failure_code
```

Обычный успешный запуск имеет persistent budget:

```text
start attempt → one NVRAM write
success       → one NVRAM clear
```