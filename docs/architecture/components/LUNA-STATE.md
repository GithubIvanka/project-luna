# `luna-state`

**Статус:** durable boundary реализована; первый backend — `redb`.

## Назначение

Хранит долговечное состояние Luna транзакционно и отделяет его от конфигурации и disposable cache.

## Владеет

- model durable system state;
- transactions;
- persistence abstraction;
- recovery/reconciliation state для долгих операций;
- schema/migration boundary.

Примеры состояния: зарегистрированные сущности, update operation state, activation metadata, durable system facts.

## Не владеет

Bundle payload, System Image filesystem, пользовательскими файлами или ephemeral cache.

## Backend

Первый принятый backend — синхронный `redb`. Backend является detail реализации storage boundary и не должен просачиваться во все верхние компоненты.

## Надёжность

Операция, признанная успешно записанной, должна сохраняться при штатном завершении процесса и корректно восстанавливаться после перезапуска. Частичная запись не должна оставлять неоднозначный lifecycle state.

## Связь с update

`luna-update-manager` оркестрирует checkpoint/rollback, а `luna-state` хранит durable operation/state data.

## Открыто

Миграции схемы, reconciliation после crash и окончательная граница между boot state, system state и recovery state.