# Контракт System Image

**Статус:** принят; структура и базовая модель разрешения ресурсов зафиксированы.  
**Scope:** versioned immutable System Image → `luna-init` → running Luna system

## 1. Назначение

Этот контракт определяет границу между версионированным System Image, его manifest, `luna-init`, DATA layer и логической файловой системой работающей Luna.

## 2. Обычный System Image

System Image — непосредственно файловая система SquashFS. `.squashfs` является самим filesystem payload.

Каноническая пара:

```text
LUNA-SYS/images/luna-X.Y.Z.squashfs
LUNA-SYS/images/luna-X.Y.Z.toml
```

Один System Image представляет одну immutable-версию минимальной Luna userspace-базы.

## 3. Внутренняя структура System Image

System Image содержит минимальный набор ресурсов и компонентов, необходимый для самостоятельного запуска базовой системы.

Каноническая верхнеуровневая структура:

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

Это logical root дерева System Image. Дополнительные верхнеуровневые каталоги не считаются частью контракта, пока не будут определены отдельным решением.

### 3.1 `apps/`

`apps/` содержит базовые системные приложения Luna, необходимые для полноценного запуска минимальной системы.

К ним относятся системные runtime, manager, updater, session-компоненты и другие приложения, входящие в базовую версию Luna.

Базовые системные приложения поставляются только в System Image и обновляются вместе с версией System Image.

DATA может содержать более новую совместимую копию конкретного системного приложения. Такая копия является DATA extension/replacement и имеет более высокий priority при разрешении пути, но базовая версия приложения остаётся частью System Image.

### 3.2 `drivers/`

`drivers/` содержит только минимально необходимый набор драйверов для запуска и работы базовой системы.

Дополнительные или более новые совместимые драйверы могут находиться в `LUNA-DATA/system/drivers/` и разрешаться с более высоким priority.

### 3.3 `firmware/`

`firmware/` — отдельная сущность и не смешивается с `drivers/`.

System Image содержит только firmware, необходимую минимальной базовой системе. Дополнительное или более новое firmware может предоставляться DATA/provider механизмом, если это допускает соответствующая driver/firmware policy.

### 3.4 `libs/`

`libs/` содержит минимальный библиотечный набор, необходимый для работоспособности System Image и его boot/runtime closure.

System Image использует musl userspace ABI как базовую модель. glibc не является обязательной частью System Image.

DATA может содержать дополнительные или более новые совместимые библиотеки. Наличие glibc в DATA используется только для компонентов, которым она действительно требуется; выбор ABI/runtime не является простым заменением musl по приоритету пути.

### 3.5 `config/`

`config/` содержит immutable defaults и базовую конфигурацию System Image.

Изменения system-wide конфигурации создаются в DATA, а индивидуальные пользовательские изменения — в соответствующем `users/<user>/config`.

### 3.6 `resources/`

`resources/` содержит immutable системные ресурсы.

Минимальный набор включает:

```text
resources/fonts/
resources/icons/
resources/themes/
resources/cursors/
resources/locales/
resources/translations/
```

Дополнительные ресурсы и более новые версии уже существующих ресурсов могут находиться в DATA.

Настройки ввода являются configuration и относятся к `config`, а не к `resources`.

## 4. DATA extension model

System Image и `LUNA-DATA/system` не являются моделью "system files" против "user files". Это два источника одного логического пространства.

System Image — минимальная immutable base.

`LUNA-DATA/system` — mutable system extension layer, содержащий дополнительные компоненты, более новые версии и system-wide changes.

Пользовательский слой находится в:

```text
LUNA-DATA/users/<user>/
```

Корень `LUNA-DATA` не содержит отдельного общего `data/` каталога в рамках принятой структуры.

## 5. Path priority

При разрешении конкретного пути DATA имеет более высокий priority, чем System Image.

Базовое правило:

```text
requested path
      ↓
LUNA-DATA candidate exists?
      ├── yes → use DATA candidate
      └── no  → use System Image candidate
```

Разрешение происходит на уровне конкретного объекта/пути, а не как безусловная замена целого каталога.

Пример:

```text
System Image:
/resources/fonts/LunaSans.ttf
/resources/fonts/LunaMono.ttf

DATA/system:
/resources/fonts/LunaSans.ttf
/resources/icons/new-set/
```

Результат:

```text
LunaSans.ttf → DATA
LunaMono.ttf → System Image
new-set/*    → DATA
```

Этот же принцип применяется к приложениям, библиотекам, драйверам, firmware и ресурсам с учётом их отдельных compatibility/security policies.

## 6. Path priority не отменяет compatibility

Высший priority означает, что DATA-кандидат рассматривается первым. Это не означает автоматическое право его использовать.

После path resolution соответствующая подсистема обязана проверить необходимые свойства кандидата, включая совместимость и policy.

Принцип:

```text
Path Resolution
      ↓
selected candidate
      ↓
Compatibility / Policy validation
      ↓
use or fallback
```

Особенно это относится к приложениям, драйверам, firmware и библиотекам.

## 7. Конфигурация

Конфигурация имеет scope-aware модель поверх общего path priority.

```text
System Image
└── config/
    └── immutable defaults

LUNA-DATA
└── system/
    └── config/
        └── system-wide changes

LUNA-DATA
└── users/
    └── <user>/
        └── config/
            └── user-specific changes
```

Если пользователь меняет системную настройку, изменяемый вариант хранится в `LUNA-DATA/system/config`.

Если изменение индивидуально для пользователя, оно хранится в `LUNA-DATA/users/<user>/config`.

## 8. Runtime state

Runtime state не является частью immutable System Image.

Каталоги и данные, создаваемые во время работы системы, materialized/generated state и прочие volatile runtime facilities не должны попадать в SquashFS System Image как persistent runtime state.

System Image содержит только immutable ресурсы и содержимое, необходимое для построения рабочего окружения.

## 9. System applications and updates

Базовые системные приложения являются частью System Image и обновляются вместе с ним.

DATA может предоставлять более новую совместимую копию отдельного системного приложения для независимого обновления. В этом случае DATA-копия имеет более высокий path priority:

```text
System Image version
        ↓ fallback
DATA version
        ↓ preferred
```

Отсутствие DATA-копии всегда оставляет базовую версию приложения доступной из System Image.

## 10. Manifest

Manifest является источником метаданных именно своего System Image payload.

Минимальная зафиксированная схема включает:

```toml
[image]
name = "luna"
version = "4.0.0"
format = "squashfs"

[architecture]
arch = "x86_64"

[init]
compatible = ["2.0.0", "2.1.0"]

[bootstrap]
critical = [
    "/sbin/luna-system-runtime",
]
```

`[bootstrap].critical` задаёт только boot-critical resources, необходимые `luna-init` до передачи управления `luna-system-runtime`.

Все пути в `critical` являются абсолютными путями внутри logical System Image root, нормализованными и безопасными для разрешения.

## 11. Boot target

Окончательная загрузочная комбинация строится как атомарный target:

```text
System Image
    ↓ compatible
luna-init
    ↓ compatible
Kernel
```

Нельзя заменять один элемент target на произвольный элемент другого target без повторной проверки полной совместимости.

## 12. Recovery System Image

Recovery имеет отдельную область на `LUNA-SYS`:

```text
LUNA-SYS/recovery/
├── recovery-X.Y.Z.squashfs
├── recovery-X.Y.Z.toml
└── ...
```

Recovery System Image — отдельный версионируемый SquashFS payload. Его lifecycle независим от обычных System Images и Factory target.

Recovery System Image может иметь собственную внутреннюю immutable структуру по тем же базовым принципам, но её состав определяется Recovery-контрактом.

## 13. Recovery DATA Image

Recovery DATA Image — отдельный versioned artifact, связанный с Recovery target.

Он предоставляет виртуальный DATA provider, materialized в RAM.

Базовая структура Recovery DATA Image:

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
└── cache/
```

Физический `LUNA-DATA` не является источником этого Recovery окружения и не требуется для запуска Recovery.

## 14. DATA abstraction

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

Для `recovery` используется VirtualData, созданный из Recovery DATA Image.

## 15. Загрузка и materialization

`luna-boot.efi` выбирает target и передаёт kernel контекст. Сам загрузчик не является владельцем логической файловой системы userspace.

`luna-init` использует выбранный System Image как immutable source для построения рабочего окружения и проверяет `[bootstrap].critical` до запуска `luna-system-runtime`.

Целевая модель:

```text
System Image
     │
     ├── immutable base
     │
     ▼
luna-init
     │
     ├── boot-critical materialization
     ├── runtime facilities
     └── logical root construction
     │
     ▼
Luna System Environment
     │
     ├── DATA/system (higher priority)
     └── DATA/users/<user>
```

System Image остаётся immutable source; DATA добавляется через resolution/mapping layer.

## 16. Целостность

До активации необходимо проверить структурную корректность payload и внутреннюю согласованность manifest. Проверка подлинности и доверия определяется отдельным security/update-контрактом.

Каждый boot target обязан сохранять идентичность конкретных Image, init и kernel, а Recovery target — также конкретного Recovery DATA Image.
