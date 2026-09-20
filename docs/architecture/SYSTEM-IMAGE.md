# Архитектура System Image

## Определение

**Краткая модель:** `SYS = boot/runtime core + default config`.

System Image — **не вся ОС Luna**, а неизменяемая версионированная минимальная userspace-файловая система конкретного релиза. Это обязательный immutable minimum для запуска boot/runtime path и достижения UserSession boundary. Графический compositor/shell не обязаны находиться внутри System Image: при normal/factory они приходят из физической `LUNA-DATA`, а при Recovery — из единственного Recovery DATA Image. `luna-init` использует System Image как immutable source, из которого материализуется работающая системная среда в RAM-backed logical `/`. `LUNA-DATA` и Recovery DATA являются отдельными provider layers.

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
    ├── sounds/
    ├── locales/
    └── translations/
```

`drivers/` и `firmware/` — разные resource classes. `drivers/` содержит driver entities/modules, `firmware/` — firmware payloads. Firmware никогда не является частью `drivers/`.

Важно различать внутренние пути источников и пути работающего logical root. Например, `config/luna/native graphical session` — это путь **внутри System Image**. Это не `/etc/luna/native graphical session` внутри образа: каталога `etc/` в структуре System Image нет. Аналогично источник изменяемого переопределения находится в физическом `LUNA-DATA/system/config/luna` configuration. Уже во время загрузки `root-mapping`/materialization строят из этих источников RAM-backed logical root и только там предоставляют приложению его логические runtime-пути.

`resources/` содержит семь типов ресурсов: `fonts/`, `icons/`, `themes/`, `cursors/`, `sounds/`, `locales/` и `translations/`. Runtime-данные конкретного системного приложения, которые должны быть доступны по стандартному пути вроде `/usr/share/<name>`, относятся к этому приложению и хранятся внутри `apps/<name>/resources/`; они не изменяют классификацию `resources/` как общего набора типов ресурсов.



`resources/sounds/` — канонический класс звуковых ресурсов System Image. Это именно ресурсный класс, а не отдельный раздел System Image. Системные звуки, необходимые базовой среде Luna и её стандартным компонентам, входят в immutable System Image; изменяемые или дополнительные звуковые наборы могут появляться в соответствующем `LUNA-DATA/system/resources/sounds/`.
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