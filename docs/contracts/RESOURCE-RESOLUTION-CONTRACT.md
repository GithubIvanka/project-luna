# Контракт Resource Resolution

**Статус:** принят как архитектурная модель; детали реализации уточняются отдельно.  
**Scope:** разрешение файлов и ресурсов между immutable System Image и LUNA-DATA

## 1. Назначение

Luna строит логическое файловое пространство из нескольких источников. System Image является immutable базой, а `LUNA-DATA` предоставляет изменяемое расширение и более новые версии ресурсов.

Resolution отвечает на вопрос:

> какой физический объект должен быть представлен по запрошенному logical path?

## 2. Источники

Для normal system основными источниками являются:

```text
System Image
LUNA-DATA/system
LUNA-DATA/users/<user>
```

System Image содержит immutable base.

`LUNA-DATA/system` содержит system-wide extensions, обновления и изменяемую системную конфигурацию.

`LUNA-DATA/users/<user>` содержит индивидуальные пользовательские данные и конфигурацию.

## 3. Priority

Для объектов, разрешаемых из System Image и DATA, более высокий priority имеет DATA.

Базовая логика:

```text
requested path
      ↓
DATA candidate exists?
      ├── yes → DATA candidate
      └── no  → System Image candidate
```

Resolution выполняется на уровне конкретного logical path, а не целой директории.

## 4. Дополнение и замещение

DATA может:

- содержать объект, которого нет в System Image;
- содержать более новую версию объекта из System Image;
- предоставлять изменённую system-wide configuration;
- предоставлять пользовательский объект в user scope.

Если для одного logical path существуют оба кандидата, DATA candidate имеет priority.

Если DATA candidate отсутствует, используется System Image candidate.

## 5. Пример

```text
System Image
├── /apps/niri
├── /libs/libA.so
├── /resources/fonts/LunaSans.ttf
└── /resources/fonts/LunaMono.ttf

LUNA-DATA/system
├── /apps/niri
├── /libs/libC.so
└── /resources/fonts/LunaSans.ttf
```

Logical result:

```text
/apps/niri                    → DATA
/libs/libA.so                 → System Image
/libs/libC.so                 → DATA
/resources/fonts/LunaSans.ttf → DATA
/resources/fonts/LunaMono.ttf → System Image
```

## 6. Resolution не означает безусловное доверие

Выбранный DATA candidate не обязан автоматически становиться исполняемым или активным.

После resolution соответствующая подсистема выполняет необходимые проверки:

```text
Path Resolution
      ↓
candidate
      ↓
Compatibility / Integrity / Security Policy
      ↓
activate or fallback
```

Это особенно важно для:

```text
apps
libs
 drivers
firmware
```

Path priority отвечает только за выбор первого кандидата.

## 7. Приложения

Базовые системные приложения поставляются в System Image.

DATA может содержать их более новую совместимую копию. Такая копия имеет priority для запуска, а базовая версия в System Image остаётся fallback.

Обновление базовой версии системного приложения происходит только вместе с обновлением соответствующего System Image.

Пользовательские приложения также могут использовать DATA как собственный источник bundle-файлов в соответствии с Application Bundle contract.

## 8. Библиотеки и ABI

Library resolution использует общий priority DATA → System Image, но ABI/runtime compatibility проверяется отдельно.

Базовым userspace ABI Luna является musl.

glibc не является обязательной частью System Image и может предоставляться DATA для приложений, которым требуется соответствующий runtime.

Наличие DATA-копии библиотеки не означает автоматическую замену библиотеки другого ABI.

## 9. Драйверы и firmware

`drivers/` и `firmware/` являются различными сущностями.

Для драйверов общий priority DATA → System Image применяется только после проверки совместимости kernel, hardware и иных обязательных свойств.

Для firmware действуют отдельные integrity/compatibility правила соответствующей подсистемы.

## 10. Configuration

Configuration resolution имеет scope поверх общего priority.

```text
System Image/config
        ↓
LUNA-DATA/system/config
        ↓
LUNA-DATA/users/<user>/config
```

System Image предоставляет immutable defaults.

`LUNA-DATA/system/config` хранит system-wide изменения.

`LUNA-DATA/users/<user>/config` хранит индивидуальные пользовательские изменения.

## 11. Resources

Тот же file-level priority применяется к ресурсам:

```text
fonts
icons
themes
cursors
locales
translations
```

Поэтому Luna может одновременно использовать ресурсы из System Image и DATA, а одинаковый logical path из DATA имеет priority.

## 12. Directory semantics

Наличие DATA объекта в одном каталоге не скрывает автоматически остальные объекты этого каталога из System Image.

Например:

```text
System Image:
/fonts/A.ttf
/fonts/B.ttf

DATA:
/fonts/A.ttf
```

Результат:

```text
/fonts/A.ttf → DATA
/fonts/B.ttf → System Image
```

Каталог рассматривается как namespace logical paths; resolution относится к отдельным объектам.

## 13. Runtime state

Runtime/generated state не является частью этого immutable overlay model.

Volatile runtime facilities создаются runtime subsystem и не должны использовать System Image как изменяемое хранилище.

## 14. Recovery

Recovery использует VirtualData, materialized из Recovery DATA Image, согласно System Image/Recovery contracts. Физический normal `LUNA-DATA` не становится скрытым источником Recovery environment только из-за наличия такого устройства.

## 15. Инварианты

1. System Image остаётся immutable.
2. DATA имеет более высокий priority при совпадении logical path.
3. Resolution выполняется на уровне конкретного объекта.
4. DATA может расширять System Image без необходимости дублировать весь каталог.
5. Compatibility, integrity и security проверки выполняются после выбора candidate.
6. Отсутствие DATA candidate возвращает resolution к System Image.
