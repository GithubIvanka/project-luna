# `luna-system-runtime`

## Назначение

Долгоживущий system-wide runtime и supervisor, который запускается `luna-init` как дочерний процесс. Он не является PID 1.

## Связь процессов

```text
PID 1
luna-init
  │
  └── luna-system-runtime
        ├── системные службы
        └── UserSession
              └── luna-app-runtime
```

## Владеет

- общесистемным supervision;
- координацией startup/shutdown после раннего bootstrap;
- коллекцией и lifecycle `UserSession`;
- координацией system managers;
- system events;
- запуском и наблюдением доверенных system processes;
- финальным подтверждением semantic boot success;
- очисткой `LunaBootAttempt` после подтверждения успеха.

## Не владеет

UEFI discovery, kernel handoff, выбором System Image, проверкой init compatibility, Bundle installation или application authorization policy.

## Успешная загрузка

В обычном boot mode runtime запускает OpenRC runlevels и ждёт фактической
готовности `luna-device-manager` и `luna-volume-manager`. Проверяются native
kernel uevent monitor, обнаруженные block partitions, включённая auto-attach
policy и хотя бы один доступный Volume. Если любой из этапов не завершился,
runtime не подтверждает boot success: ошибка остаётся явной и вход в графическую
сессию не начинается. Только после этой проверки подтверждается `SUCCESS`, и
`LunaBootAttempt` удаляется через `efivarfs`.

Recovery mode сохраняет отдельную последовательность с виртуальным DATA provider.

```text
SystemRuntime started
      ↓
runtime + system state ready
      ↓
semantic SUCCESS
      ↓
clear LunaBootAttempt
```

## Жизненный цикл

```text
spawned by luna-init
  ↓
starting
  ↓
running
  ↓
system/session supervision
  ↓
shutdown or reboot
```

## PID 1 boundary

`luna-init` остаётся PID 1 и продолжает reap дочерних процессов независимо от lifecycle `luna-system-runtime`.

## Ошибки

Падение runtime не создаёт нового supervisor layer. `luna-init` остаётся PID 1 и применяет установленную failure/reboot/recovery policy.

## Статус

Process supervision, UserSession integration и durable state integration присутствуют в коде. Полная production startup orchestration, lifecycle reconciliation и resource limits ещё требуют реализации.