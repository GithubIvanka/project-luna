# Контракт разрешения ресурсов

## Канонические источники

```text
immutable base → выбранный System Image
mutable system → LUNA-DATA/system
user state     → LUNA-DATA/users/<user>
cache          → LUNA-DATA/cache
```

Разрешение выбирает допустимый источник для конкретного логического ресурса. Физические пути `LUNA-SYS/...` и `LUNA-DATA/...` являются деталями хранения и не входят в API приложения.

Только resource classes, для которых явно разрешено переопределение, могут получать источник из DATA поверх immutable base.

Bundle/application declarations используют только логические пути.

## Последовательность

```text
требования ресурсов
      ↓
MappingPlan
      ↓
luna-root-mapping
      ↓
luna-security
      ↓
разрешённый mapping
      ↓
luna-namespace
      ↓
materialization
```

Mapping определяет допустимый источник. Security отдельно определяет, разрешено ли его использовать. Namespace реализует только разрешённый результат.

## Правила mapping

Каждый `ApplicationInstance` имеет собственное mapping state. Нет одной глобальной mapping table для всех приложений.

Основная гранулярность — отдельный файл. Subtree mapping используется только там, где он соответствует семантике resource class.

Конфликт mappings внутри одного namespace является ошибкой. Молчаливое переопределение не допускается.

## Recovery

Recovery использует `VirtualData`, материализованный из Recovery DATA Image. Физическая `LUNA-DATA` при этом остаётся отдельным объектом для диагностики и ремонта.

## Drivers и firmware

`drivers/` и `firmware/` являются разными resource classes и во всех утверждённых системных DATA representation находятся в отдельных каталогах.

```text
system/
├── drivers/
└── firmware/
```

`drivers/` содержит driver entities/modules. `firmware/` содержит firmware payloads. Firmware никогда не является дочерним каталогом `drivers/`. То же разделение действует внутри System Image и Recovery DATA Image.