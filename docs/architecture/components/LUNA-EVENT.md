# `luna-event`

**Статус:** domain boundary реализована; транспортная и durable integration продолжаются.

## Назначение

Типизированный домен событий Luna и контракты подписки/доставки.

## Владеет

- event identities/types;
- subscriptions;
- delivery contracts;
- правилами lifecycle подписки.

## Правило

Событие описывает факт, а не скрытую команду. Нельзя помещать в event payload произвольный mutable global state.

Примеры:

```text
DeviceAdded
DeviceRemoved
VolumeMounted
UserLoggedIn
UserLoggedOut
ApplicationStarted
ApplicationExited
SystemUpdated
KernelChanged
```

Разные domain events должны иметь отдельные типизированные payloads, а не один универсальный JSON/string payload.

## Не владеет

IPC transport, durable state storage, authorization, device discovery или process supervision.

## Зависимости

Минимальные shared identifiers и выбранный transport layer.

## Открыто

IPC backend, durability/replay semantics и интеграция с desktop/system runtime.