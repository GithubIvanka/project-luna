# `luna-kernel-manager`

**Статус:** inventory/build direction реализованы частично; boot/update integration продолжается.

## Назначение

Ведёт inventory Linux kernels и предоставляет metadata/compatibility semantics для других компонентов.

## Владеет

- kernel inventory;
- metadata;
- version comparison;
- compatibility queries;
- lifecycle install/remove в рамках kernel domain;
- подготовкой данных для update path.

## Не владеет

UEFI handoff, System Image payload, application runtime или System Image update transaction.

## Совместимость

Совместимость определяется явно отношением image ↔ kernel. Нельзя объявлять kernel совместимым только по версии.

## Интеграция

`luna-boot.efi` использует kernel compatibility information при выборе boot target. `luna-update-manager` оркестрирует изменение состояния, а этот компонент предоставляет domain operations.

## Ошибки

Недействительное или несовместимое ядро не должно становиться active choice. Ошибка установки не должна удалять текущую рабочую версию.

## Открыто

Точная metadata schema, integrity/authenticity, boot-state integration и retention kernels.