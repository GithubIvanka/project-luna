# Жизненный цикл выполнения

## Владение процессами

```text
PID 1
└── luna-init
    └── luna-system-runtime
        ├── системные службы
        ├── UserSession
        │   └── luna-app-runtime
        │       └── ApplicationInstance → процесс
        └── другие контролируемые системные процессы
```

`luna-init` — начальный userspace-процесс и остаётся PID 1. Он выполняет раннюю загрузку и запускает `luna-system-runtime` как дочерний процесс.

## Запуск системы

После запуска `luna-system-runtime` он загружает постоянное системное состояние, инициализирует утверждённые системные менеджеры, создаёт инфраструктуру событий/устройств и запускает графическую границу входа. `luna-init` при этом остаётся PID 1.

Успех загрузки подтверждается `luna-system-runtime` только после достижения согласованной границы инициализации runtime и system state; после этого очищается `LunaBootAttempt`.

## UserSession

`UserSession` — доменная сущность. Она начинается через login flow, получает `ACTIVE` после успешной аутентификации, может перейти в `RESTRICTED` при переключении пользователя и завершается состоянием `TERMINATED`.

Несколько сессий могут существовать одновременно.

## Жизненный цикл приложения

`luna-app-runtime` строит и сопровождает один `ApplicationInstance`.

```text
запрос запуска
      ↓
планирование
      ↓
mapping
      ↓
authorization
      ↓
materialization
      ↓
запуск процесса
      ↓
выполнение
      ↓
завершение / ошибка
      ↓
очистка
```

`ApplicationPlan` и `MappingPlan` являются данными одного запуска. `luna-root-mapping` отвечает за mapping, `luna-security` — за trust и authorization, а `luna-namespace` — за Linux materialization и настройку изолированной среды.

Для конкретного процесса в `RuntimeSpec` выбирается одна libc: `musl` или `glibc`.

System runtime владеет общесистемным supervision. App runtime владеет lifecycle `ApplicationInstance`. Дополнительный application supervisor или второй init не создаётся.

## Завершение

Завершение системы инициируется system runtime. Пользовательские приложения и сессии останавливаются согласно policy, системные службы завершаются, необходимые durable state сохраняются, после чего выполняется reboot/power transition.