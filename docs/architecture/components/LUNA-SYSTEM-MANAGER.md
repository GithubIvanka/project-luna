# `luna-system-manager`

**Статус:** boundary/scaffold; durable system model ещё развивается.

## Назначение

Предоставляет domain model и запросы к состоянию установленной Luna, не исполняя сами state-changing transactions.

## Владеет

- System Image inventory semantics;
- system status/query model;
- представлением current/factory и доступных версий;
- системными metadata operations, не принадлежащими bootloader.

## Не владеет

UEFI boot, kernel process loading, application lifecycle, Bundle codec или update transaction execution.

## Update boundary

`luna-update-manager` меняет состояние. `luna-system-manager` предоставляет domain-level view этого состояния.

## Зависимости

`luna-state`, image/kernel domain contracts и необходимые shared types.

## Ошибки

Запрос неизвестной сущности должен давать typed not-found/error semantics, а не скрываться пустым результатом, если отсутствие означает нарушение ожидаемого contract.

## Открыто

Полная system inventory model, activation semantics и reconciliation с Boot State.