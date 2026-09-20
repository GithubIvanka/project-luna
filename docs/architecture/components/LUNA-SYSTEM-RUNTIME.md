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

После запуска runtime и инициализации необходимого system state он подтверждает `SUCCESS`. Только после этого `LunaBootAttempt` удаляется через `efivarfs`.

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