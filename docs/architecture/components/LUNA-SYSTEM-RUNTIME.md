# `luna-system-runtime`

**Статус:** основное ядро реализовано; production-интеграция не завершена.

## Назначение

Единый системный runtime и supervisor. Он координирует `UserSession` и наблюдает за runtime-активностью, не поглощая ответственность managers.

## Что принадлежит компоненту

- lifecycle системного runtime;
- supervised process lifecycle;
- реестр и orchestration `UserSession`;
- создание, аутентификация и завершение `UserSession`;
- привязка supervised runtime activity к сессиям;
- запуск и координация графической desktop-сессии внутри активного `UserSession`.

## Иерархия

```text
luna-system-runtime
├── UserSession A
│   ├── luna-app-runtime
│   └── GUI/Desktop session
└── UserSession B
    ├── luna-app-runtime
    └── GUI/Desktop session
```

Отдельного Luna session manager и компонента `luna-run-session` нет.

## Контракт сессии

Графическая login-сессия начинается в состоянии аутентификации. Только после успешной аутентификации `UserSession` может стать `Active`. Запуск графической сессии для неактивной `UserSession` должен отклоняться.

## Граница приложения

`luna-app-runtime` владеет lifecycle `ApplicationInstance` и подготовкой среды выполнения. `luna-system-runtime` предоставляет системную границу supervision и не превращается в application manager.

## Идентичность пользователя

Linux-механизмы или helper-программы могут использоваться для установки идентичности и окружения активного `UserSession`. Такие helper'ы являются деталями реализации и не образуют новых Luna-компонентов.

## Что НЕ принадлежит компоненту

- установка Bundle;
- политика авторизации;
- raw filesystem mapping;
- UEFI boot;
- inventory/compatibility kernel;
- реализация desktop shell.

## Зависимости

`luna-user-session`, Linux process primitives, event/state contracts и необходимые lower-level security/namespace contracts.

## Сборка

Из корня репозитория:

```bash
tools/build-component.sh luna-system-runtime --release
```

Или напрямую:

```bash
cargo build --release -p luna-system-runtime
```

Для PC-образа используется musl target:

```bash
cargo build --release -p luna-system-runtime --target x86_64-unknown-linux-musl
```

Результат:

```text
target/release/luna-system-runtime
target/x86_64-unknown-linux-musl/release/luna-system-runtime
```

## Открытые вопросы

Требуют дальнейшей реализации: production privilege transition, полноценное переключение/ограничение нескольких сессий, durable supervision/reconciliation и финальный authentication IPC.
