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

В обычном boot mode runtime напрямую запускает и supervises `luna-device-manager`
и `luna-volume-manager`. Внешний service manager не является обязательным для
этого критического пути. Runtime ждёт readiness обоих компонентов и проверяет:
наличие native kernel uevent monitor, обнаруженных block partitions, включённой
auto-attach policy и хотя бы одного доступного Volume. Если любой этап не
завершился, runtime не подтверждает boot success, а графическая сессия не
начинается.

После успешного gate runtime подтверждает `SUCCESS` и очищает `LunaBootAttempt`.
Только затем, если доступен `/usr/bin/openrc`, запускаются внешние compatibility
runlevels для некритичных desktop/system providers. OpenRC не запускает Luna
Device Manager или Volume Manager и не владеет их readiness.

Recovery mode сохраняет отдельную последовательность с виртуальным DATA provider
и не требует обычного physical-volume startup.

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