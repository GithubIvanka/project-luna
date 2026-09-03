# `luna-audio`

**Статус:** domain boundary реализована; provider integration неполная.

## Назначение

Предоставляет Luna audio domain независимо от конкретного desktop implementation.

## Владеет

- audio state;
- моделью endpoint/device;
- операциями volume и routing на границе Luna;
- provider abstraction.

Ключевые domain concepts: `Volume`, `AudioState`, `AudioEndpoint`.

## Не владеет

Внутренностями PipeWire, authorization policy, GUI widgets, lifecycle UserSession или общим device discovery.

## Provider

Текущий PC image содержит PipeWire, PipeWire-Pulse и WirePlumber как infrastructure. Сам факт их упаковки не доказывает полной integration Luna audio provider.

## Зависимости

Общие domain values, security/session context при необходимости и выбранный Linux audio stack.

## Открыто

D-Bus/provider integration, per-user routing, session lifecycle и управление из Noctalia.