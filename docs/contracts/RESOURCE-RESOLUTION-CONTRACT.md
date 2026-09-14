# Контракт Resource Resolution

**Статус:** принят как архитектурная модель; детали реализации уточняются отдельно.  
**Scope:** разрешение файлов и ресурсов между immutable System Image и LUNA-DATA

## 1. Назначение

Luna строит логическое файловое пространство из нескольких источников. System Image является immutable базой, а `LUNA-DATA` предоставляет изменяемое расширение и более новые версии разрешённых ресурсов.

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

`LUNA-DATA/system` содержит system-wide extensions, дополнительные компоненты, разрешённые обновляемые системные ресурсы и изменяемую системную конфигурацию.

`LUNA-DATA/users/<user>` содержит индивидуальные пользовательские данные и конфигурацию.

## 3. Priority

Для сущностей, которым разрешено разрешение между System Image и DATA, более высокий priority имеет DATA.

Базовая логика:

```text
requested path
      ↓
LUNA-DATA candidate exists?
      ├── yes → DATA candidate
      └── no  → System Image candidate
```

Resolution выполняется на уровне конкретного logical path, а не целой директории.

## 4. Дополнение и замещение

DATA может:

- содержать объект, которого нет в System Image;
- содержать более новую или изменённую версию объекта, если его DATA replacement разрешён policy;
- предоставлять изменённую system-wide configuration;
- предоставлять пользовательский объект в user scope.

Если для одного logical path существуют оба кандидата, DATA candidate имеет priority только для DATA-replaceable сущности.

Если DATA candidate отсутствует, используется System Image candidate.

## 5. Пример

```text
System Image
├── /libs/libA.so
├── /resources/fonts/LunaSans.ttf
└── /resources/fonts/LunaMono.ttf

LUNA-DATA/system
├── /libs/libC.so
└── /resources/fonts/LunaSans.ttf
```

Logical result:

```text
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

## 7. Системные и пользовательские приложения

Базовые системные приложения поставляются только в System Image.

Они являются частью конкретной версии System Image и обновляются только вместе с обновлением System Image.

`LUNA-DATA/system` не может содержать replacement-копию базового системного приложения и не является механизмом его независимого обновления.

Поэтому для базового системного приложения отсутствует DATA → System Image replacement path:

```text
System Image
└── base system application
        ↓
   authoritative source
```

Пользовательские приложения используют DATA как собственный источник Bundle-файлов в соответствии с Application Bundle contract.

DATA может также содержать дополнительные системные компоненты, если отдельный contract явно определяет такую сущность как DATA-managed; это не распространяется автоматически на базовые системные приложения System Image.

## 8. Библиотеки и ABI

Library resolution использует общий priority DATA → System Image для DATA-replaceable библиотек, но ABI/runtime compatibility проверяется отдельно.

Базовым userspace ABI Luna является musl.

glibc не является обязательной частью System Image и может предоставляться DATA для приложений, которым требуется соответствующий runtime.

Наличие DATA-копии библиотеки не означает автоматическую замену библиотеки другого ABI.

## 9. Драйверы и firmware

`drivers/` и `firmware/` являются различными сущностями.

Для drivers общий priority DATA → System Image применяется только после проверки совместимости kernel, hardware и иных обязательных свойств, а конкретные DATA replacements должны быть разрешены driver policy.

Для firmware действуют отдельные integrity/compatibility правила соответствующей подсистемы.

System Image содержит firmware, необходимую минимальной базовой системе.

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

Тот же file-level priority применяется к DATA-replaceable ресурсам:

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

Runtime/generated state не является частью этого immutable resolution model.

Volatile runtime facilities создаются runtime subsystem и не должны использовать System Image как изменяемое хранилище.

## 14. Recovery

Recovery использует VirtualData, materialized из Recovery DATA Image, согласно System Image/Recovery contracts. Физический normal `LUNA-DATA` не становится скрытым источником Recovery environment только из-за наличия такого устройства.

## 15. Инварианты

1. System Image остаётся immutable.
2. Базовые системные приложения принадлежат только System Image и обновляются только вместе с ним.
3. DATA имеет более высокий priority только для сущностей, которым разрешён DATA replacement.
4. Resolution выполняется на уровне конкретного logical path.
5. DATA может расширять System Image без необходимости дублировать весь каталог.
6. Compatibility, integrity и security проверки выполняются после выбора candidate.
7. Отсутствие DATA candidate возвращает resolution к System Image для DATA-replaceable сущностей.
