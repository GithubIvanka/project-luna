# Boot Attempt Contract

**Статус:** Accepted  
**Версия:** 1  
**Scope:** `luna-boot` → Linux kernel → `luna-init` → `luna-system-runtime`

## Назначение

`BootAttempt` описывает один конкретный запуск Luna от момента запуска `luna-boot` до подтверждённого успеха системы.

Главный принцип:

> подробный progress живёт в RAM; persistent storage содержит только минимальный marker, необходимый для обнаружения незавершённой попытки после reboot, panic или power loss.

Это предотвращает постоянные перезаписи boot-state во время нормальной загрузки.

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

Отдельные стадии являются volatile runtime state и не требуют записи на диск при каждом переходе.

## Volatile progress

Общая модель находится в `luna-common`:

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

`BootAttempt` в `luna-boot` использует эту модель как runtime-only состояние.

## Persistent marker

Persistent marker хранится в UEFI variable storage.

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

Persistent marker создаётся один раз перед передачей управления kernel:

```text
all boot objects prepared
        ↓
write in_progress marker
        ↓
ExitBootServices
        ↓
kernel
```

Если запись marker невозможна, запуск не должен продолжаться как будто crash detection работает.

## Обнаружение незавершённой попытки

При следующем запуске `luna-boot` читает marker.

Если существует валидный marker `in_progress`, это означает:

```text
previous attempt did not reach SUCCESS
```

Причина может быть любой:

- kernel panic;
- reset;
- power loss;
- failure before userspace success confirmation.

Отдельно выяснять причину persistent marker не требуется.

`BOOT_STATE.previous_attempt_failed` в Luna Handoff должен отражать наличие такого незавершённого marker.

## Attempt ID

Каждая новая попытка получает собственный `attempt_id`.

Если предыдущий persistent marker существует, новый ID получается последовательным увеличением предыдущего ID.

Если marker отсутствует, ID детерминированно создаётся из параметров текущего подготовленного запуска с BLAKE3 и не может быть нулевым.

ID сохраняется:

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

persistent `in_progress` marker должен быть удалён/закрыт.

Пока success reporter не реализован, marker намеренно остаётся persistent после старта kernel/userspace. Это позволяет следующим загрузкам обнаруживать незавершённую попытку.

## Failure

Если failure возникает до `ExitBootServices` и Luna может безопасно сохранить диагностическую информацию, она может быть отражена в отдельном failure state.

При kernel panic после `ExitBootServices` запись marker уже не требуется: сам факт наличия `in_progress` marker является признаком незавершённой попытки.

## Relationship with Boot State

`BootAttempt` и `boot-state.toml` имеют разные роли.

`boot-state.toml` хранит persistent boot policy/context и atomic targets:

```text
current
fallback
recovery
factory
```

`BootAttempt` отвечает только за текущий запуск и его crash-detection marker.

Detailed stage progress не должен превращаться в постоянный журнал в `boot-state.toml`.

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

## Write minimization

В обычном успешном запуске persistent boot-attempt state должен изменяться только на необходимых границах:

```text
start attempt → one persistent write
success       → one persistent clear/close
```

Все промежуточные стадии остаются в памяти.

