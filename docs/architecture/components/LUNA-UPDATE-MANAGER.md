# `luna-update-manager`

**Статус:** transaction foundation реализован; полноценная mutation/reconciliation/rollback integration продолжается.

## Назначение

Оркестрирует state-changing обновления Luna и согласует их с `luna-state`, System Image, kernel и checkpoint/rollback semantics.

## Владеет

- update transaction lifecycle;
- staging;
- activation coordination;
- checkpoint/rollback orchestration;
- reconciliation после незавершённых операций;
- безопасным retention после подтверждения здоровья.

## Не владеет

Самим Bundle codec, low-level kernel inventory, UEFI loader, GUI или process supervision.

## System Image update

```text
obtain
  ↓
verify
  ↓
stage
  ↓
leave current intact
  ↓
activate
  ↓
reboot
  ↓
health confirmation
  ↓
commit / rollback
```

Неуспешное обновление не должно уничтожать последнюю подтверждённую рабочую версию.

## Независимость

Обновление System Image не должно требовать обновления kernel, если старое kernel совместимо. И наоборот.

## State

Долгоживущие transaction facts хранятся через `luna-state`. После crash менеджер должен уметь определить незавершённую операцию и безопасно её reconciliate.

## Открыто

Health gating, финальная transaction state machine, independent kernel update path, automatic rollback и Recovery/Factory integration.