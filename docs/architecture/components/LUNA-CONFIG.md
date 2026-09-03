# `luna-config`

**Статус:** foundation реализован; финальная persistence/serialization integration продолжается.

## Назначение

Отвечает за модель конфигурации Luna и правила области действия настроек.

## Владеет

- типизированными configuration values;
- scoped configuration;
- загрузкой/сохранением человекочитаемой конфигурации там, где это предусмотрено;
- правилами precedence между уровнями конфигурации.

TOML является предпочтительным форматом для человекочитаемой конфигурации и metadata там, где это уместно.

## Не владеет

Durable system state, Bundle Format, System Image payload, kernel metadata или application authorization.

## Граница config/state

```text
config → что настроено
state  → какое устойчивое состояние имеет система
```

`luna-config` не должен использоваться как универсальное хранилище operational state.

## Ошибки

Malformed configuration должна приводить к понятной ошибке загрузки/валидации. Безопасные значения по умолчанию могут применяться только там, где это заранее определено policy.

## Зависимости

TOML/serialization primitives и минимальные shared types.

## Открыто

Финальная schema registry, persistence integration, precedence policy и миграции.