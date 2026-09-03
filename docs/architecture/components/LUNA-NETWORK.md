# `luna-network`

**Статус:** domain boundary реализована; provider integration продолжается.

## Назначение

Предоставляет Luna network domain независимо от конкретного network daemon.

## Владеет

- model network interfaces/connections;
- connection state;
- профильными настройками на границе Luna;
- запросами connect/disconnect;
- provider abstraction и событиями сети.

## Не владеет

Низкоуровневым kernel networking, общей device discovery, GUI widgets, authorization policy или UserSession lifecycle.

## Provider

Текущий PC image использует NetworkManager как Linux provider infrastructure. Это не означает, что его внутренний API становится частью Luna architecture.

## Security

Policy доступа приложения к сети задаётся через `luna-security`; наличие network interface не означает автоматический доступ каждого приложения.

## Открыто

NetworkManager/D-Bus integration, network state events, connection UI и policy enforcement.