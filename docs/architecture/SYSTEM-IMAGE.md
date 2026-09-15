# Архитектура System Image

## Определение

System Image — неизменяемая версионированная userspace-файловая система конкретного релиза Luna. Это источник, из которого `luna-init` материализует работающую системную среду в RAM-backed logical `/`.

```text
LUNA-SYS/images/
├── luna-X.Y.Z.squashfs
└── luna-X.Y.Z.toml
```

`.squashfs` является самой System Image. Внешней оболочки вокруг неё нет. `.lbp` относится только к Bundle.

## Внутренняя структура

```text
/
├── apps/
├── drivers/
├── firmware/
├── libs/
├── config/
└── resources/
    ├── fonts/
    ├── icons/
    ├── themes/
    ├── cursors/
    ├── locales/
    └── translations/
```

`drivers/` и `firmware/` — разные resource classes. `drivers/` содержит driver entities/modules, `firmware/` — firmware payloads. Firmware никогда не является частью `drivers/`.

`state/` и `volumes/` отсутствуют намеренно: это mutable DATA concerns и они находятся в `LUNA-DATA/system`.

## Манифест

Файл `luna-X.Y.Z.toml` находится рядом с образом и читается до открытия SquashFS. Он описывает identity/version image и совместимые `luna-init` cores.

Точная TOML grammar определяется контрактом System Image. Реализация не должна добавлять неподтверждённые поля ради удобства.

## Совместимость

Полный target разрешается только так:

```text
System Image
    ↓ manifest image
совместимый luna-init
    ↓ manifest init
совместимый kernel
```

System Image не задаёт прямую image → kernel compatibility.

## Материализация

System Image остаётся immutable source на `LUNA-SYS`. `luna-init` материализует boot-critical system resources в RAM-backed logical root. Весь image целиком в RAM не копируется; дополнительные immutable resources могут загружаться лениво.

Уже материализированные ресурсы должны работать независимо от дальнейшего существования pathname исходного image.

## Жизненный цикл

```text
build
  ↓
validate
  ↓
install в LUNA-SYS
  ↓
discovery
  ↓
select
  ↓
boot как immutable source
  ↓
rollback / retention
  ↓
retire
```

System Image не становится writable после успешной загрузки. Mutable изменения принадлежат `LUNA-DATA`.

## Обновление

Обновление System Image не означает автоматического обновления kernel или `luna-init`. Все артефакты остаются независимо версионированными, но перед загрузкой обязаны образовать совместимый полный target.

## Recovery

Recovery не имеет отдельной System Image. Используется обычный выбранный System Image, совместимые `luna-init` и kernel, а дополнительным recovery-источником является только отдельный Recovery DATA Image.