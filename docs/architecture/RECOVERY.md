# Архитектура Recovery и Factory

## Цели загрузки

`current`, `factory` и `recovery` являются полными boot targets. Target никогда не собирается смешиванием несовместимых артефактов.

```text
current  → System Image + luna-init + kernel
factory  → System Image + luna-init + kernel
recovery → System Image + luna-init + kernel + Recovery DATA Image
```

## Хранилище Recovery

Recovery-специфичные данные находятся в `LUNA-SYS/recovery/`.

```text
LUNA-SYS/recovery/
├── recovery.squashfs
└── recovery.toml
```

Это **Recovery DATA Image**, а не Recovery System Image. Recovery не имеет отдельной системной SquashFS: используется обычный выбранный System Image, а `luna-init` и kernel разрешаются той же цепочкой `image → init → kernel`, что и при normal boot.

## Recovery DATA Image

Recovery DATA Image — отдельный версионированный SquashFS-источник, который во время Recovery materialization становится `VirtualData` в RAM.

```text
Recovery DATA Image
├── system/
│   ├── apps/
│   │   ├── recovery-tools/
│   │   ├── data-discovery/
│   │   ├── system-diagnostics/
│   │   └── system-repair/
│   ├── drivers/
│   ├── firmware/
│   ├── libs/
│   ├── config/
│   ├── state/
│   └── volumes/
├── users/
│   └── recovery/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

`drivers/` и `firmware/` — отдельные resource classes и не объединяются. `state/` сохраняет Recovery state. `volumes/` нужен для логического состояния управляемых внешних томов.

## DATA provider

```text
Normal / Factory
    PhysicalData
       ↓
    LUNA-DATA

Recovery
    VirtualData
       ↓
    Recovery DATA Image → RAM
```

Recovery может стартовать без физической `LUNA-DATA`.

В Recovery не запускается интерактивный desktop login provider. `luna-init` передаёт `luna-system-runtime` идентичность виртуального пользователя `recovery`, а runtime непосредственно создаёт активный Recovery UserSession. Поэтому Recovery DATA не обязана содержать `greetd` или Noctalia Greeter: её графический вход — тот же Niri provider, что и в Normal/Factory, с native seat/session ownership Luna.

## Доступ к физической DATA

Физическая `LUNA-DATA` во время Recovery является объектом диагностики и ремонта, а не backing store работающей Recovery-среды.

Recovery может:

- находить диски и кандидатов `LUNA-DATA`;
- проверять и диагностировать DATA;
- при нескольких валидных кандидатах просить пользователя выбрать нужный;
- исправлять или заменять DATA binding;
- записывать выбранные disk GUID и partition GUID в `LUNA-SYS/config/luna-data.toml`.

## Переход в Recovery

Если normal boot не может получить однозначную валидную физическую `LUNA-DATA`, `luna-boot.efi` автоматически выбирает Recovery target для продолжения загрузки.

Recovery не повторно использует физическую DATA как provider. Он использует `VirtualData`, созданную из Recovery DATA Image.

## Factory

`factory` — сохранённый заводской известный хороший target:

```text
System Image + luna-init + kernel
```

Factory не является shell fallback и не вводит отдельный runtime layer. Retention policy не должна случайно удалять factory target.