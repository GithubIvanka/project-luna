# Контракт System Image

**Статус:** принят как базовый контракт; детали materialization/hydration уточняются отдельной реализацией.  
**Scope:** System Image → `luna-init` → running RAM-backed system

## 1. Назначение

Этот контракт определяет границу между версионированными System Images, manifest-файлами, загрузчиком, `luna-init`, DATA abstraction, Recovery Environment и подсистемами обновления.

## 2. Обычный System Image

System Image — непосредственно файловая система SquashFS.

Каноническая пара:

```text
LUNA-SYS/images/luna-X.Y.Z.squashfs
LUNA-SYS/images/luna-X.Y.Z.toml
```

`.squashfs` является самим filesystem payload.

## 3. Recovery System Image

Recovery имеет отдельную область на `LUNA-SYS`:

```text
LUNA-SYS/recovery/
├── recovery-X.Y.Z.squashfs
├── recovery-X.Y.Z.toml
└── ...
```

Recovery System Image — отдельный версионируемый SquashFS payload. Его lifecycle независим от обычных System Images и Factory target.

Recovery manifest описывает собственную идентичность и совместимость с `luna-init`. Manifest выбранного `luna-init` определяет совместимые kernels.

```text
Recovery System Image
        ↓ compatible luna-init
luna-init
        ↓ compatible kernel
Kernel
```

Recovery System Image может обновляться независимо, например для добавления новых средств диагностики, восстановления и системного обслуживания.

## 4. Идентичность

Для обнаруженного образа должны быть однозначно определимы:

- имя семейства или Recovery role;
- версия `X.Y.Z`;
- архитектура;
- путь к payload;
- путь к соответствующему manifest.

Несоответствие имени, версии или manifest приводит к отказу от использования образа.

## 5. Manifest

Manifest является источником метаданных именно для своего payload. Минимальный набор семантик:

- идентичность и версия;
- архитектура;
- формат `squashfs`;
- совместимые `luna-init` версии;
- параметры, необходимые для передачи управления ядру;
- сведения о целостности, если они определены политикой доверия.

Manifest System Image не определяет kernels напрямую. Совместимость kernel является ответственностью manifest выбранного `luna-init`.

Пример обычного Image manifest:

```toml
[image]
name = "Luna"
version = "4.0.0"
format = "squashfs"

[architecture]
arch = "x86_64"

[init]
compatible = ["2.0.0", "2.1.0"]
```

Точная TOML-схема будет отдельным контрактом. Реализация загрузчика не должна расширять смысл полей молча.

## 6. Boot target

Окончательная загрузочная комбинация строится как атомарный target:

```text
System Image
    ↓ compatible
luna-init
    ↓ compatible
Kernel
```

Recovery target имеет тот же принцип и дополнительно содержит ссылку на Recovery DATA Image.

Нельзя заменять один элемент target на произвольный элемент другого target без повторной проверки полной совместимости.

## 7. DATA abstraction

System Image не содержит пользовательский физический DATA.

Luna использует единую DATA abstraction с двумя provider-типами:

```text
DATA
├── PhysicalData
│   └── physical LUNA-DATA
│
└── VirtualData
    └── Recovery DATA Image → RAM
```

Для `current` и `factory` DATA provider — физический `LUNA-DATA`.

Для `recovery` `luna-init` сначала материализует указанный Recovery DATA Image в RAM и представляет получившуюся среду как DATA.

Остальная система использует одну и ту же логическую DATA interface и не должна различать normal и Recovery только из-за способа хранения DATA.

## 8. Recovery DATA Image

Recovery DATA Image — отдельный versioned artifact, связанный с Recovery target.

Он имеет логическую структуру DATA:

```text
Recovery DATA Image
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── config/
│   └── ...
├── users/
│   └── recovery/
│       ├── home/
│       ├── data/
│       └── config/
├── data/
└── cache/
```

Recovery DATA Image содержит программы, libraries, configuration и временное пользовательское окружение Recovery.

Физический `LUNA-DATA` не является источником этого окружения и не требуется для запуска Recovery.

## 9. Работа Recovery с физическим DATA

После создания RAM-backed DATA Recovery может работать с физическими DATA-кандидатами как с отдельными системными объектами.

Recovery utilities могут:

- сканировать физические диски;
- находить и валидировать `LUNA-DATA`;
- показывать несколько кандидатов;
- выбирать нужный DATA;
- обновлять binding configuration через контролируемый системный механизм;
- диагностировать и восстанавливать DATA.

Recovery DATA discovery не использует `luna-data.toml` для определения того, откуда взять собственную DATA-среду.

## 10. Загрузка и материализация

`luna-boot.efi` выбирает target и передаёт kernel контекст. Сам загрузчик не является владельцем Linux filesystem materialization.

`luna-init` использует выбранный System Image как immutable source для построения рабочего окружения:

```text
System Image
     │
     ▼
luna-init
     │
     ├── RAM: boot-critical system base
     ├── RAM: runtime directories and pseudo-filesystems
     └── lazy hydration of additional immutable system content
                     ↓
             RAM-backed logical `/`
```

Для Recovery дополнительно:

```text
Recovery DATA Image
        │
        ▼
   materialization
        │
        ▼
 VirtualData in RAM
```

VirtualData становится обычным DATA provider для дальнейшего запуска системы.

## 11. Независимость lifecycle

Обычные System Images, Factory target, Recovery System Image и Recovery DATA Image имеют отдельные lifecycle и retention rules.

Обновление Recovery может происходить независимо от обновления обычного System Image.

Recovery DATA Image также может обновляться независимо для расширения диагностического и восстановительного инструментария.

## 12. Взаимодействие с System State

Persistent System State содержит:

```text
current
├── image
├── init
└── kernel

factory
├── image
├── init
└── kernel

recovery
├── image
├── init
├── kernel
└── data
```

`recovery.data` идентифицирует Recovery DATA Image, которую необходимо материализовать в RAM при запуске Recovery.

## 13. Целостность

До активации необходимо проверить структурную корректность payload и внутреннюю согласованность manifest. Проверка подлинности и доверия определяется отдельным security/update-контрактом.

Каждый boot target обязан сохранять идентичность конкретных Image, init и kernel, а Recovery target — также конкретного Recovery DATA Image.
