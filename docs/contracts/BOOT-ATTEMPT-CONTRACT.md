# Boot Attempt Contract

**Статус:** Accepted  
**Версия:** 1  
**Scope:** `luna-boot` → Linux kernel → `luna-init` → `luna-system-runtime`

## Назначение

`BootAttempt` описывает один конкретный запуск Luna от момента запуска `luna-boot` до подтверждённого успеха системы.

Главный принцип:

> подробный progress живёт в RAM; persistent storage содержит только минимальный marker, необходимый для обнаружения незавершённой попытки после reboot, panic или power loss.

Это предотвращает постоянные перезаписи `boot-state.toml` во время нормальной загрузки.

## Жизненный цикл

```text
UEFI
  ↓
luna-boot loaded
  ↓
luna-boot completed
  ↓
kernel handoff
  ↓
kernel started
  ↓
luna-init started
  ↓
luna-init ready
  ↓
luna-system-runtime started
  ↓
SUCCESS
```

Подробные стадии являются volatile runtime state и не требуют записи на диск при каждом переходе.

## Volatile progress

`BootAttemptProgress` и `BootAttempt` принадлежат `luna-boot`, потому что bootloader работает как `no_std` UEFI-приложение и runtime progress является его локальным состоянием.

Модель:

```text
BootAttemptProgress
├── attempt_id
└── stage
```

Допустимые стадии:

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

Стадия монотонна: переход назад запрещён.

Этот progress хранится только в RAM текущего запуска.

## Persistent marker

Persistent marker хранится в UEFI variable storage (NVRAM), а не в `LUNA-SYS/config/boot-state.toml`.

Текущая реализация использует:

```text
Variable name: LunaBootAttempt
State: in_progress
```

Marker содержит только минимальные данные:

```text
magic
format
status
attempt_id
checksum
```

Размер фиксирован и мал.

## Запись marker

Для каждой новой попытки загрузки `luna-boot` выполняет одну persistent-запись marker в NVRAM после того, как все необходимые boot objects подготовлены, и непосредственно перед `ExitBootServices`:

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
    ↓
kernel
```

Промежуточные стадии между этими точками не записываются в persistent storage.

Если запись marker невозможна, запуск не должен продолжаться так, будто crash detection гарантирован.

## Обнаружение незавершённой попытки

При следующем запуске `luna-boot` читает marker из NVRAM.

Если существует валидный marker `in_progress`, это означает:

```text
previous attempt did not reach SUCCESS
```

Причина может быть любой:

- kernel panic;
- reset;
- power loss;
- failure before userspace success confirmation.

Отдельно выяснять причину по самому marker не требуется.

`BOOT_STATE.previous_attempt_failed` в Luna Handoff должен отражать наличие такого незавершённого marker.

## Attempt ID

Каждая новая попытка получает собственный `attempt_id`.

Если предыдущий persistent marker существует, новый ID получается последовательным увеличением предыдущего ID.

Если marker отсутствует, ID детерминированно создаётся из параметров текущего подготовленного запуска с BLAKE3 и не может быть нулевым.

ID сохраняется в runtime handoff:

```text
persistent marker
        ↓
Luna Handoff
        ↓
luna-init / system runtime
```

## Success

Успех не считается достигнутым только потому, что `luna-init` был запущен.

Финальную семантическую проверку выполняет `luna-system-runtime`.

Только после подтверждения:

```text
attempt N → SUCCESS
```

`luna-system-runtime` удаляет persistent `LunaBootAttempt` marker из NVRAM.

Таким образом:

```text
marker отсутствует → предыдущая загрузка завершилась успешно
marker in_progress  → предыдущая загрузка не дошла до SUCCESS
```

Пока success reporter не реализован, marker намеренно остаётся persistent после старта kernel/userspace. Это позволяет следующим загрузкам обнаруживать незавершённую попытку.

## Failure

Если failure возникает до `ExitBootServices` и Luna может безопасно сохранить диагностическую информацию, она может быть отражена в отдельном failure state.

После `ExitBootServices` bootloader больше не изменяет UEFI state. При kernel panic persistent `in_progress` marker просто остаётся существовать и будет обнаружен следующей загрузкой.

## Relationship with Boot State

`BootAttempt` и `boot-state.toml` имеют разные роли.

`boot-state.toml` хранит долгоживший boot context и atomic targets:

```text
current
fallback
recovery
factory
```

`BootAttempt` отвечает только за конкретный текущий запуск и crash-detection marker.

Подробный runtime progress не должен превращаться в постоянный журнал в `boot-state.toml`.

## Relationship with Luna Handoff

`Luna Handoff ABI v1` передаёт `attempt_id` как идентичность текущего запуска.

`BOOT_STATE` передаёт persistent context предыдущей попытки:

```text
previous_attempt_failed
previous_attempt_id
fallback_depth
failure_code
```

Это не журнал всех runtime stages.

## Persistent write budget

В обычном успешном запуске persistent Boot Attempt state изменяется только на необходимых границах:

```text
start attempt → one NVRAM write
success       → one NVRAM clear
```

Все промежуточные стадии остаются в RAM.
