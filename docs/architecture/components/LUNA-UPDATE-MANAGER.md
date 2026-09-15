# `luna-update-manager`

## Назначение

Оркестрирует транзакционные операции обновления, checkpoint и rollback, не становясь реализацией storage.

## Владеет

- update plans и фазами;
- durable revision-aware состоянием операций;
- созданием checkpoint;
- выполнением подтверждённых update operations через backend;
- reconciliation незавершённых операций;
- изменением durable boot targets через `luna-system-manager`.

## Не владеет

Форматом System Image, компиляцией ядра, выбором UEFI target или lifecycle процесса приложения.

## Жизненный цикл

```text
prepare → checkpoint → apply → verify → commit
                    ↘ failure / recovery
```

## Boot targets

При изменении системы сохраняется полная identity target:

```text
current  = System Image + luna-init + kernel
factory  = System Image + luna-init + kernel
recovery = System Image + luna-init + kernel + Recovery DATA Image
```

System Image, `luna-init` и kernel остаются независимо версионированными, но commit/rollback выполняются для совместимого полного target.

## Подтверждение обновления

Новый target не считается подтверждённым только потому, что его файлы записаны. Успех требует semantic runtime startup, после которого `luna-system-runtime` очищает `LunaBootAttempt`, а `luna-update-manager`/`luna-system-manager` фиксируют подтверждённый current target.

## Ошибка и восстановление

Незавершённая update operation должна быть обнаружена по durable state и либо безопасно завершена, либо приведена к согласованному состоянию до следующего conflicting update.

## Статус

Planning, transactional state и test backends существуют. Полные artifact/filesystem mutation backends ещё интегрируются.