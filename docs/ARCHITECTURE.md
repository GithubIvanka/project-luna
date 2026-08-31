# Project Luna

## Architecture Source of Truth & Development Guide

**Проект:** Project Luna
**Внутреннее имя:** `luna`
**Язык разработки:** Rust
**Лицензия:** Apache License 2.0
**Модель проекта:** операционная система Luna с минимальной неизменяемой системной основой, использующая Linux kernel
**Статус документа:** архитектурный Source of Truth
**Назначение:** единая база для разработки Project Luna и для отдельных чатов по компонентам проекта

---

# 0. Назначение этого документа

Этот документ является главным архитектурным контекстом Project Luna.

Он нужен для того, чтобы при продолжении разработки в другом чате не возникала ситуация, когда часть ранее принятых решений забывается, смешивается с новыми предложениями или заменяется альтернативной архитектурой.

Каждый отдельный чат, посвящённый компоненту Luna, должен рассматривать этот документ как исходную архитектурную базу.

Главное правило:

> Если решение помечено как **ПРИНЯТО**, нельзя молча менять его во время разработки отдельного компонента.

Если реализация компонента показывает, что принятое решение технически невозможно или имеет серьёзную архитектурную проблему, это нужно сначала вынести как отдельный архитектурный вопрос.

Нельзя просто заменить решение новым вариантом и продолжить разработку так, будто старого решения не существовало.

---

# 1. Философия Project Luna

Project Luna создаётся на основе идеи One File Linux.

Главная идея:

> система должна иметь очень маленькую, стабильную и максимально неизменяемую основу.

Luna не должна строиться как обычный Linux-дистрибутив с изменённым набором пакетов, оформлением и менеджером пакетов. Linux kernel является ядром, на котором работает Luna, а архитектура userspace, системного образа, загрузки, приложений и управления системой определяется самим проектом Luna.

Luna должна отличаться архитектурой.

Основные принципы:

1. Маленькая системная основа.
2. Максимальная неизменяемость системы.
3. Версионированные System Images.
4. Независимое обновление системы и ядра.
5. Приложения в виде bundles.
6. Изолированные зависимости.
7. Минимальное количество директорий верхнего уровня.
8. Собственный загрузчик Luna.
9. Автоматическое управление внешними устройствами.
10. Изоляция приложений через существующие Linux-механизмы.
11. Управление системой через единую утилиту `luna`.
12. Максимально простой пользовательский интерфейс системы.
13. Система должна уметь восстанавливаться после неудачного обновления.
14. Пользовательские данные не должны зависеть от конкретной версии System Image.

---

# 2. Что Luna НЕ должна быть

Luna не должна быть:

* системой, построенной как обычный Linux-дистрибутив;
* Docker-подобной ОС;
* системой с огромным количеством каталогов в корне;
* системой, где каждое приложение устанавливает файлы по всему `/`;
* системой, где обновление ядра требует переписывать всю систему;
* системой, где обновление ОС уничтожает пользовательские данные;
* системой, где пользователь постоянно должен вручную монтировать флешки через терминал;
* системой, где bootloader каждый запуск переписывает boot state;
* системой, где все ядра перебираются без проверки совместимости;

---

# 3. Базовая модель файловой системы

Мы сознательно не хотим повторять классическую Linux-структуру с большим количеством директорий.

На уровне диска у Luna четыре основных раздела:

```text
Disk
├── EFI
├── system
├── data
└── swap
```
`SWAP` is optional and may be implemented as a partition, file or ZRAM.
`EFI` and `SYSTEM` are OS-managed. `DATA` is the normal mutable/user-visible storage area.

Это ПРИНЯТО.

---

# 4. Раздел EFI

`EFI` предназначен только для загрузочной инфраструктуры.

В нём находится собственный загрузчик Luna:

```text
EFI/
└── Luna/
    └── luna-boot.efi
```

Главный загрузчик:

```text
luna-boot.efi
```

---

# 5. luna-boot.efi

ПРИНЯТО:

> функциональность, относящуюся непосредственно к UEFI boot flow, переносим в `luna-boot.efi`.

Основная модель:

```text
UEFI
 ↓
luna-boot.efi
 ↓
Linux kernel
 ↓
Luna System Image
```

---

# 6. Обычная загрузка

Luna должна загружаться максимально незаметно.

Пользователь не должен видеть:

* долгий boot menu;
* лишние сообщения;
* ненужные задержки.

Обычный сценарий:

```text
Power
 ↓
UEFI
  ↓
luna-boot.efi
  ↓
selected compatible kernel
  ↓
minimal RAM logical-root environment
  ↓
selected System Image
  ↓
attach DATA
  ↓
luna-system-runtime
  ↓
UserSession(s)
```

---

# 7. Boot Menu

Boot Menu вызывается только при специальном действии пользователя.

Принято использовать клавишу:

```text
B
```

То есть:

```text
Power
 ↓
UEFI
 ↓
luna-boot.efi
 ↓
B pressed?
 ├── NO  → normal boot
 └── YES → Boot Menu
```

Boot Menu должно позволять:

* продолжить загрузку ос;
* выбрать System Image и предложить совместимое ядро;
* перейти в recovery;
* перейти в factory;
* загрузиться с USB/другого внешнего носителя;
* выполнять другие системные recovery/boot операции.

---


# 8. Раздел system

Раздел `system` содержит версии Luna System Images и Linux kernels.

Каноническая структура:

```text
system/
├── images/
│   ├── luna-1.0.0.squashfs
│   ├── luna-1.0.0.toml
│   ├── luna-2.0.0.squashfs
│   ├── luna-2.0.0.toml
│   └── ...
│
└── kernels/
    ├── 7.0.0
    ├── 8.0.0
    └── ...
```

В `system/images/` каждый System Image хранится вместе со своим manifest.

Это ПРИНЯТО:

---

# 9. System Image

System Image — это версия самой Luna.

Например:

```text
luna-1.0.0
luna-2.0.0
luna-3.0.0
```

System Image хранится как файл:

```text
luna-1.0.0.squashfs
```

---

# 10. Формат System Image

System Image является непосредственно SquashFS filesystem image.

Формат файла:

```text
luna-X.Y.Z.squashfs
```

Например:

```text
luna-3.0.0.squashfs
```

В `system/images/` рядом с каждым образом находится его manifest: `luna-X.Y.Z.toml`.

Таким образом, каждая версия представлена парой:

```text
luna-3.0.0.squashfs
luna-3.0.0.toml
```

`.squashfs` — это непосредственно файловая система SquashFS, используемая как System Image.

Это один из главных архитектурных инвариантов проекта.

---

# 11. Почему SquashFS

SquashFS выбран потому, что хорошо соответствует идее Luna:

* read-only;
* сжатый;
* подходит для System Image;
* позволяет хранить целую системную файловую структуру;
* удобен для versioned immutable system images;
* позволяет отделить системную файловую систему от пользовательских данных;
* подходит для нашей идеи загрузки нужных частей системы по мере необходимости.

---

# 12. Bundle Format и System Image

Luna использует два независимых формата.

Bundle Format:

```text
.lbp
```

System Image:

```text
.squashfs
```

Bundle Format используется для bundles, а SquashFS — для версионированных System Images.

---

# 13. Manifest каждого System Image

Каждый System Image имеет собственный manifest.

Это ПРИНЯТО.

Нельзя использовать только один глобальный manifest для всех System Images как единственный источник информации об образах.

Структура:

```text
system/
└── images/
    ├── luna-1.0.0.squashfs
    ├── luna-1.0.0.toml
    │
    ├── luna-2.0.0.squashfs
    ├── luna-2.0.0.toml
    │
    ├── luna-3.0.0.squashfs
    ├── luna-3.0.0.toml
    │
    └── ...
```

Manifest относится именно к соответствующему образу.

---

# 14. Для чего нужен manifest

`luna-boot.efi` должен иметь возможность узнать информацию об образе без необходимости сначала загружать сам System Image.

Manifest должен в дальнейшем описывать как минимум концепции:

* имя;
* версию;
* архитектуру;
* формат;
* совместимые kernels;
* необходимые boot parameters;
* другую информацию, необходимую bootloader.

Точный синтаксис manifest ещё должен быть формально описан.

Например, концептуально:

```toml
[image]
name = "luna"
version = "3.0.0"
format = "squashfs"

[architecture]
arch = "x86_64"

[kernels]
compatible = [
    "8.0.0",
    "8.1.0",
    "8.2.0",
]
```

Это НЕ финальная спецификация.

Не считать этот пример уже утверждённым форматом.

---

# 15. Manifest рядом с образом

Manifest хранится непосредственно рядом с соответствующим System Image, чтобы `luna-boot.efi` мог прочитать metadata образа до его загрузки.

Структура:

```text
system/images/
├── luna-3.0.0.squashfs
├── luna-3.0.0.toml
├── luna-2.0.0.squashfs
├── luna-2.0.0.toml
└── ...
```

---

# 16. Linux kernels

Ядра также относятся к `system` и хранятся в каталоге `kernels`.

Например:

```text
system/
└── kernels/
    ├── 7.0.0
    ├── 7.1.0
    ├── 8.0.0
    ├── 8.1.0
    └── 8.2.0
```

Имя каталога для ядер является ПРИНЯТЫМ:

> `system/kernels/`

---

# 17. Независимость System Image и kernel

Одно из фундаментальных решений:

> обновление System Image не должно требовать обновления kernel.

И наоборот:

> обновление kernel не должно требовать переписывания System Image.

Например:

```text
System Image:
luna-3.0.0

Kernel:
8.2.0
```

можно обновить независимо.

Это позволяет:

* откатывать систему;
* откатывать kernel;
* тестировать новый kernel;
* сохранять рабочий System Image;
* сохранять рабочее ядро при обновлении другой части.

---

# 18. Маленький неизменяемый boot layer

Мы обсуждали идею максимально маленькой неизменяемой основы.

Изначальная идея была похожа на One File Linux:

```text
очень маленькая основа
        ↓
передача управления
        ↓
текущая версия kernel/system
```


Поэтому текущая модель:

```text
UEFI
 ↓
маленький luna-boot.efi
 ↓
выбранный Linux kernel
 ↓
System Image
```

Нужно избегать превращения `luna-boot.efi` в огромную ОС.

---

# 19. current

`current` — текущая используемая система.

Концептуально:

```text
current
    image = luna-3.0.0
    kernel = 8.2.0
```

Точный физический формат `current` ещё предстоит определить.

Главное семантическое правило:

> `current` указывает на текущую рабочую комбинацию System Image + kernel.

---

# 20. factory

`factory` — заводское состояние системы.

Это отдельная концепция.

При первоначальной установке:

```text
factory
    image = luna-1.0.0
    kernel = 7.0.0
```

После обновлений:

```text
factory
    image = luna-1.0.0
    kernel = 7.0.0

current
    image = luna-5.0.0
    kernel = 9.0.0
```

`current` может изменяться.

`factory` должен сохраняться.

---

# 21. Зачем нужен factory

`factory` — последняя гарантированная точка восстановления.

Если произошла настолько серьёзная ошибка, что:

* текущий образ не запускается;
* fallback kernels не помогают;
* другие совместимые System Images не запускаются;

можно вернуться к заводскому состоянию.

Концептуально:

```text
current
 ↓
compatible fallback
 ↓
older image
 ↓
older compatible kernel
 ↓
no working combination
 ↓
factory
```

---

# 22. Soft fallback

Soft fallback используется для отказа System Image при уже выбранном и загруженном kernel.

Если при попытке запуска System Image произошла ошибка и Luna System не смогла стартовать, `luna-boot.efi` должен попытаться загрузить предыдущую совместимую версию System Image без перезагрузки компьютера.

Пример:

```text
Luna 3.0.0 + Kernel 8.2.0
        ↓
   System failure
        ↓
Luna 2.0.0 + Kernel 8.2.0
        ↓
      success
```

Если предыдущая версия также не запускается, bootloader продолжает поиск среди более ранних System Images, совместимых с текущим kernel.

Kernel panic является отдельным сценарием. При kernel panic компьютер перезагружается, после чего `luna-boot.efi` выбирает предыдущее совместимое kernel и выполняет обычную загрузку.

Таким образом:

```text
System Image failure
    ↓
предыдущий совместимый System Image
    ↓
без reboot

Kernel panic
    ↓
reboot
    ↓
предыдущее совместимое kernel
```

---

# 23. Совместимость kernels

Нельзя просто перебрать все установленные kernels.

Если выбран:

```text
Luna 3.0.0
```

Bootloader сначала читает:

```text
luna-3.0.0.toml
```

и получает:

```text
compatible kernels
```

После чего показывает пользователю только их.

Например:

```text
Luna 3.0.0

> Linux 8.2.0
  Linux 8.1.0
  Linux 8.0.0
```

а не все kernels, установленные в системе.

---

# 24. Fallback должен учитывать compatibility

Нельзя делать:

```text
Image A
 ↓
Kernel 1
 ↓
FAIL
Kernel 2
 ↓
FAIL
Kernel 3
 ↓
FAIL
```

если эти kernels не совместимы с Image A.

Правильно:

```text
Image A
 ↓
read manifest
 ↓
compatible kernels
 ↓
try only those kernels
```

После исчерпания compatible kernels можно перейти к другому System Image.

---

# 25. Boot state

Мы отдельно приняли решение:

> boot state не должен переписываться при каждом обычном запуске.

Например, если Luna успешно загрузилась 20 раз подряд, нет смысла каждый раз изменять состояние на диске.

Boot state изменяется только при событиях.

Примеры событий:

* установка новой системы;
* удаление системы;
* установка kernel;
* удаление kernel;
* смена `current`;
* успешное подтверждение новой версии;
* зафиксированная ошибка загрузки;
* изменение recovery state;
* изменение другой информации, действительно влияющей на boot.

Обычный boot:

```text
read
 ↓
boot
```

а не:

```text
read
 ↓
rewrite
 ↓
rewrite
 ↓
rewrite
```

---

# 26. Максимальное количество System Images

Мы решили, что количество хранимых System Images должно быть настраиваемым.

Например, пользователь может захотеть:

```text
4 последних версии
```

плюс:

```text
factory
```

То есть factory не должна уничтожаться обычной политикой очистки.

Точная конфигурация retention policy ещё должна быть специфицирована.

---

# 27. Раздел data

`data` предназначен для изменяемого состояния.

Текущая принятая структура:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

Это ВАЖНО.

---

# 28. DATA/system

В:

```text
DATA/system/
```

находятся изменяемые компоненты пользовательской системы:

`DATA/system/apps` stores installed application Bundles shared between users.
`DATA/system/drivers` stores OS-managed mutable driver content.
`DATA/system/libs` stores shared library content.
`DATA/system/volumes` stores managed external-volume state.
`DATA/system/config` stores machine-wide mutable configuration.
`DATA/system/state` stores persistent system state.

То есть здесь находятся:

* установленные приложения;
* драйверы;
* соответствующие изменяемые системные компоненты.

---

# 29. Applications

Главная идея приложения Luna:

> приложение должно быть похоже на macOS `.app`.

Приложение представляет собой отдельную директорию/bundle, содержащую необходимые файлы.

Например концептуально:

```text
data/system/apps/
└── some-app/
    ├── manifest
    ├── bin/
    ├── lib/
    ├── resources/
    └── ...
```

Приложение не должно бесконтрольно разбрасывать свои файлы по системному корню.

---

# 30. Libraries

Библиотеки должны быть организованы так, чтобы избежать конфликтов зависимостей.

Мы хотим модель, концептуально похожую на Nix:

```text
library A
library B
library C
```

каждая зависимость изолирована.

При этом Luna не должна превращаться в копию NixOS.

Идея, которую мы берём:

> зависимости должны быть адресуемыми и изолированными, а не конфликтовать в одной глобальной куче файлов.

---

# 31. Bundle Format

Для приложений и других устанавливаемых компонентов мы выбрали собственный формат:

```text
.lbp
```

Название:

> Luna Bundle Package / Luna Bundle Package-like format

Точное расшифрование названия ещё не является отдельным утверждённым стандартом, поэтому в документации пока безопаснее говорить просто:

> Luna Bundle Format

---

# 32. Bundle Format и System Image — разные подсистемы

Luna использует два независимых формата.

Bundle Format:

```text
.lbp
```

System Image:

```text
.squashfs
```

Bundle Format используется для bundles, а SquashFS — для версионированных System Images.

# 33. RFC-0002

Следующая большая спецификация:

```text
RFC-0002 — Bundle Format v1
```

Она должна описать `.lbp`.

В ней нужно определить:

* структуру bundle;
* metadata;
* manifest;
* payload;
* версии;
* идентификаторы;
* зависимости;
* архитектуру;
* целевую платформу;
* установку;
* обновление;
* удаление;
* проверку целостности;
* подписи, если они будут приняты;
* совместимость;
* правила хранения;
* формат файлов внутри bundle.

Но пока нельзя считать конкретный формат `.lbp` окончательно определённым.

---

# 34. System Image Specification

После Bundle Format необходимо отдельно специфицировать System Image.

Потому что System Image — не bundle.

Нужно будет определить:

```text
System Image
+
per-image manifest
+
kernel compatibility
+
boot metadata
+
versioning
+
retention
```

При этом сам payload:

```text
luna-X.Y.Z.squashfs
```

остаётся SquashFS.

---

# 35. Доступ пользователя к system

Мы обсуждали идею, что пользователь не должен напрямую работать с системным разделом как с обычной пользовательской файловой системой.

Это соответствует общей философии:

```text
system
    ↓
OS-managed
```

а:

```text
data
    ↓
user/application-managed
```

Детали монтирования `system` и специальных административных инструментов ещё требуют отдельной спецификации.

Не придумывать сейчас конкретные команды или permissions без RFC.

---

# 36. Drivers

Драйверы находятся в:

```text
data/system/drivers/
```

Каждый драйвер должен быть отдельной сущностью.

Концептуально:

```text
data/system/drivers/
├── driver-a/
├── driver-b/
└── driver-c/
```

Это позволяет изолированно управлять драйверами.

Если конкретный драйвер ломает систему, recovery должен иметь возможность отключить/удалить его без уничтожения всей системы.

---

# 37. External Devices

Мы отдельно приняли UX-идею, похожую на Windows.

Пользователь подключает:

```text
USB flash drive
```

и не должен писать вручную:

```bash
mount /dev/sdb1 /mnt/usb
```

Ожидаемое поведение:

```text
USB inserted
 ↓
device detected
 ↓
filesystem detected
 ↓
automount
 ↓
file manager
 ↓
new volume appears
```

То есть внешний накопитель должен автоматически появляться в файловом менеджере.

Это не означает, что Luna должна копировать внутреннюю реализацию Windows.

Принято именно пользовательское поведение:

> подключил устройство → оно автоматически появилось в проводнике.

Конкретный backend ещё предстоит выбрать и реализовать.

---

# 38. Container technology

Мы не хотим создавать собственную контейнерную платформу уровня ОС.

Не нужно делать:

```text
Luna
 ↓
собственный Docker
 ↓
контейнеры
```

Это противоречит принципу использования существующих системных механизмов.

Мы хотим использовать существующие Linux container technologies, работающие поверх Linux kernel.

---

# 39. Mount namespaces

Нам понравилась идея mount namespaces.

Концептуальная модель:

```text
Application A
    ↓
mount namespace A

Application B
    ↓
mount namespace B
```

Приложение может видеть собственную файловую среду и не должно автоматически видеть весь host filesystem.

Идея:

> каждое приложение получает контролируемое filesystem view.

Для приложения может выглядеть так:

```text
/
├── app
├── lib
├── data
└── ...
```

а реальная структура host system остаётся скрыта.

---

# 40. Доступ приложений к пользовательским файлам

Мы отдельно отметили, что нужно продумать:

* как приложение получает доступ к `Documents`;
* как оно получает файл от пользователя;
* как пользователь отдаёт файл приложению;
* как приложение получает доступ к внешнему диску;
* как ограничивать доступ к другим директориям;
* как делать file picker;
* как передавать данные между application namespace и user namespace.

Это пока **ОТДЕЛЬНАЯ НЕЗАКРЫТАЯ ОБЛАСТЬ**.

Не считать, что она уже окончательно спроектирована.

---

# 41. Desktop environment

Выбранное пользовательское окружение:

```text
niri
+
Noctalia Shell
```

Это является выбранным направлением desktop environment.

---

# 42. Terminal

Выбранная комбинация:

```text
Ghostty
+
fish
```

---

# 43. Init / service management

Мы обсуждали использование чего-то похожего на OpenRC.

Направление:

> использовать существующую лёгкую Linux service/init систему или архитектуру, похожую на OpenRC, вместо создания всего с нуля.

Точный выбор и интеграция ещё должны быть оформлены отдельно.

Не считать, что мы уже реализовали собственный init/service manager.

---

# 44. Один инструмент luna

Большая UX-идея проекта:

```text
luna
```

должна стать основным инструментом управления системой.

Концептуально:

```text
luna
```

может управлять:

* System Images;
* bundles;
* applications;
* drivers;
* updates;
* kernels;
* recovery;
* system configuration;
* storage;
* возможно, service management.

Но конкретный CLI ещё не полностью специфицирован.

---

# 45. TOML

Мы выбрали TOML для конфигурационных файлов и metadata там, где это удобно.

Причина:

* читаемый человеком;
* простой;
* хорошо подходит Rust ecosystem;
* удобен для конфигурации;
* не требует сложного формата.

Но нельзя автоматически считать, что абсолютно каждый формат Luna обязан быть TOML.

Например:

```text
System Image payload
```

— SquashFS.

```text
Bundle
```

— отдельная спецификация `.lbp`.

Manifest/config — там, где это принято архитектурой, может быть TOML.

---

# 46. Rust

Основной язык разработки:

```text
Rust
```

Это ПРИНЯТО.

Cargo используется как основной build system/package/workspace tooling для Rust-кода.

---

# 47. Репозиторий

Проект находится в личном Git repository.

Название:

```text
Project Luna
```

Внутренние имена компонентов используют:

```text
luna
```

Лицензия:

```text
Apache License 2.0
```

---

# 51. Логическая декомпозиция проекта

Этот раздел описывает архитектурные подсистемы Luna, а не текущую структуру каталогов репозитория.

Логическая декомпозиция нужна для того, чтобы крупные части системы можно было проектировать и разрабатывать независимо друг от друга. Она не означает, что для каждой подсистемы уже должен существовать отдельный Cargo component или каталог.

```text
Project Luna
│
├── Boot
│   └── luna-boot.efi
│
├── System Images
│   ├── SquashFS images
│   ├── manifests
│   ├── versioning
│   ├── kernel compatibility
│   └── rollback
│
├── Kernel Management
│   ├── kernel storage
│   ├── compatibility
│   └── selection
│
├── Bundle System
│   ├── .lbp format
│   ├── manifest
│   ├── dependencies
│   └── installation
│
├── Filesystem
│   ├── filesystem abstraction
│   ├── mount handling
│   └── namespaces
│
├── Application Runtime
│   ├── application isolation
│   ├── namespaces
│   └── permissions
│
├── Device Management
│   ├── USB
│   ├── external storage
│   └── automount
│
├── Services
│   └── service management
│
├── Configuration
│   └── TOML/configuration
│
├── CLI
│   └── luna
│
└── Desktop / Session
    ├── niri
    ├── Noctalia Shell
    ├── Ghostty
    └── fish
```

## 51.1 Правило для репозитория

Архитектурная подсистема создаётся в репозитории только тогда, когда начинается её реальная разработка. Пустые каталоги и Cargo components для будущих подсистем заранее не создаются.

Следующие компоненты не нужно создавать заранее только потому, что они присутствуют в архитектурной схеме. Они появляются в workspace в момент начала их разработки.

Таким образом, существует чёткое различие:

```text
Архитектура
    ↓
описывает все подсистемы будущей Luna

Репозиторий
    ↓
содержит только реально разрабатываемые компоненты
```

---

# 52. Рекомендуемая независимость компонентов

Главный принцип разработки:

> каждый компонент должен иметь минимальное количество знаний о других компонентах.

Например:

`luna-bundle` не должен знать детали bootloader.

`luna-boot` не должен знать, как устроен GUI.

`luna-config` не должен знать внутренности SquashFS.

`luna-fs` не должен содержать логику Bundle Format.

---

# 53. Компонент: luna-common

Назначение:

Общие типы и фундаментальные структуры, используемые несколькими компонентами.

Туда могут попасть:

* version types;
* identifiers;
* архитектурные enums;
* общие metadata structures.

Но:

> `luna-common` не должен превращаться в свалку всего проекта.

Если конкретная структура относится только к bundle — она должна находиться в bundle component.

Если относится только к boot — она не должна автоматически попадать в common.

---

# 54. Компонент: luna-log

Назначение:

Общая система логирования Luna.

Необходимо отделить:

* boot-time logging;
* system logging;
* application logging;

если архитектура это потребует.

Особенно важно помнить, что `luna-boot.efi` имеет совершенно другую среду исполнения, чем обычный Linux userspace.

Не предполагать автоматически, что код обычного `luna-log` можно использовать внутри UEFI.

---

# 55. Компонент: luna-config

Назначение:

Конфигурация Luna.

Связан с:

* TOML;
* system configuration;
* retention settings;
* пользовательскими настройками;
* другими конфигурационными данными.

Но boot-critical metadata должна иметь чётко определённый формат и lifecycle.

Не следует превращать `luna-config` в универсальную библиотеку для всех файлов проекта без архитектурной причины.

---

# 56. Компонент: luna-fs

Назначение:

Filesystem-related functionality.

Потенциальные области:

* filesystem abstraction;
* mount operations;
* filesystem detection;
* mount namespaces;
* storage;
* external volumes.

Но device automount можно выделить в отдельный logical subsystem, если он станет достаточно большим.

---

# 57. Компонент: luna-bundle

Назначение:

Реализация `.lbp`.

Этот компонент должен развиваться вокруг:

```text
Bundle Format v1
```

Он не должен включать:

* System Image boot logic;
* kernel selection;
* bootloader;
* SquashFS System Image semantics.

---

# 58. Компонент: luna-cli

`luna` — основной CLI/управляющий интерфейс.

Концептуально:

```text
luna <command>
```

Он должен стать пользовательским способом управления Luna.

В будущем сюда могут попасть команды вроде:

```text
luna system ...
luna bundle ...
luna kernel ...
luna driver ...
luna device ...
luna recovery ...
```

Но конкретный CLI пока не утверждён.

Не придумывать окончательный набор команд до отдельного проектирования.

---

# 59. Boot component

Архитектурно:

```text
luna-boot.efi
```

это отдельный компонент.

Он должен быть максимально маленьким.

Основные обязанности:

1. взаимодействие с UEFI;
2. обнаружение клавиши `B`;
3. запуск обычной загрузки;
4. Boot Menu;
5. обнаружение System Images;
6. чтение manifests;
7. определение compatible kernels;
8. выбор kernel;
9. выбор System Image;
10. fallback;
11. recovery entry;
12. загрузка Linux kernel.

Он НЕ должен:

* быть полноценной userspace ОС;
* управлять GUI;
* заниматься package management;
* управлять приложениями;

---

# 60. System Image component

Отдельная логическая подсистема.

Она должна отвечать за:

* создание System Image;
* проверку SquashFS;
* metadata;
* manifest;
* versioning;
* installation;
* removal;
* retention;
* verification;
* связь Image ↔ compatible kernels.

---

# 61. Kernel component

Отдельная логическая подсистема.

Отвечает за:

* хранение kernels;
* kernel metadata;
* compatibility;
* installation;
* removal;
* selection;
* fallback.

Kernel management не должен быть жёстко связан с application bundle management.

---

# 62. Device Manager

Отдельная логическая подсистема.

Отвечает за:

```text
device detected
 ↓
filesystem detected
 ↓
mount
 ↓
volume exposed to desktop
```

Особое внимание:

* USB;
* external disks;
* removable media;
* filesystem detection;
* safe removal;
* mount permissions.

---

# 63. Application Runtime

Это отдельный большой компонент.

Он должен решать:

```text
Application
 ↓
filesystem namespace
 ↓
allowed paths
 ↓
user data
 ↓
external devices
```

Здесь будут использоваться существующие Linux-механизмы.

Особенно важны:

```text
mount namespaces
```

---

# 64. Application permissions

Нужно будет разработать понятную модель:

```text
Application
    ↓
может видеть
    ↓
только необходимые ресурсы
```

Но при этом пользователь должен иметь удобный способ:

* открыть файл;
* сохранить файл;
* выбрать папку;
* получить доступ к USB;
* передать файл приложению.

Это отдельная архитектурная задача.

---

# 65. Service Manager

Мы хотим использовать существующие Linux решения, а не писать собственную систему сервисов без необходимости.

Ориентир:

```text
OpenRC-like
```

Но окончательная архитектура ещё не специфицирована.

---

# 66. Desktop layer

Desktop layer не должен смешиваться с core architecture.

Выбрано:

```text
niri
Noctalia Shell
Ghostty
fish
```

Эти компоненты являются частью пользовательского окружения, но не определяют фундаментальную архитектуру Luna.

---

# 67. Что считается стабильным фундаментом

На данный момент к наиболее важным архитектурным инвариантам относятся:

```text
Project Luna
    ↓
Luna operating system
    ↓
minimal immutable system foundation
    ↓
Linux kernel
    ↓
EFI / system / data / swap
    ↓
custom luna-boot.efi
    ↓
versioned System Images
    ↓
System Image = SquashFS
    ↓
one manifest per System Image
    ↓
independent kernels
    ↓
kernel compatibility
    ↓
current
    ↓
factory
    ↓
soft fallback
    ↓
.lbp bundles for applications/components
```

---

# 68. Что НЕ считать окончательно определённым

Следующие вещи пока нельзя выдавать за готовую спецификацию:

* точная структура kernel metadata;
* точная процедура определения kernel panic;
* точная процедура soft fallback;
* точный device automount backend;
* точная OpenRC integration;
* окончательный CLI `luna`;

Если новый чат обсуждает эти вещи, он должен сначала проектировать их, а не считать уже существующими решениями.

---

# 69. Hybrid loading

Мы обсуждали идею, что System Image не должен бездумно загружаться целиком в RAM.

Причина:

System Image может быть большим.

Мы хотим:

```text
small initial load
        ↓
boot quickly
        ↓
load required data
        ↓
other data becomes available when needed
```

Поэтому понравился hybrid approach:

* критически необходимая часть доступна сразу;
* остальная часть системы может подгружаться по мере необходимости.

SquashFS хорошо подходит для этой модели.

Но точная реализация ещё НЕ определена.

Не утверждать, что весь System Image обязательно копируется в RAM.

---

# 70. One File Linux inspiration

One File Linux для нас является архитектурным источником идеи:

> маленькая неизменяемая основа и простой способ запуска системы.

Мы не обязаны копировать One File Linux буквально.

Luna развивается в сторону:

```text
minimal immutable core
+
versioned system images
+
separate kernels
+
application bundles
+
mutable data
```

---

# 71. Структура диска — итог

Каноническая структура:

```text
Disk
│
├── EFI
│   └── Luna
│       └── luna-boot.efi
│
├── system
│   ├── images
│   │   ├── luna-1.0.0.squashfs
│   │   ├── luna-1.0.0.toml
│   │   ├── luna-2.0.0.squashfs
│   │   ├── luna-2.0.0.toml
│   │   ├── luna-3.0.0.squashfs
│   │   ├── luna-3.0.0.toml
│   │   └── ...
│   │
│   └── kernels
│       ├── 7.0.0
│       ├── 8.0.0
│       ├── 8.1.0
│       └── ...
│
├── data
│   ├── system/
│   │   ├── apps/
│   │   ├── drivers/
│   │   ├── libs/
│   │   ├── volumes/
│   │   ├── config/
│   │   └── state/
│   │
│   ├── users/
│   │   └── <user>/
│   │       ├── home/
│   │       ├── data/
│   │       └── config/
│   └── cache
│
└── swap
```

---

# 73. Как начинать отдельный чат

Каждый новый чат по Luna должен начинаться с краткого контекста.

Рекомендуемый заголовок:

```text
Я работаю над Project Luna.

Используй следующий документ как Source of Truth:
Project Luna — Architecture Source of Truth & Development Guide

Главные инварианты:
- Rust
- minimal immutable Luna system foundation
- EFI/system/data/swap
- custom luna-boot.efi
- System Images находятся в system
- System Image = непосредственно SquashFS
- каждый System Image имеет отдельный manifest
- kernels находятся в system
- current = текущая система
- factory = заводское состояние
- soft fallback использует только совместимые kernels
- .lbp = отдельный Bundle Format
- applications/drivers находятся в data/system
- users/data/cache находятся в data
- mount namespaces используются для application isolation
- внешние накопители должны автоматически появляться в desktop
- niri + Noctalia Shell
- Ghostty + fish
- Rust/Cargo workspace

Компонент, который мы сейчас обсуждаем:
[НАЗВАНИЕ КОМПОНЕНТА]

Не меняй архитектурные инварианты молча.
Если предлагаемое решение требует изменения принятой архитектуры, сначала явно укажи это как архитектурный конфликт.
```

---

# 74. Отдельный чат: luna-boot

Для чата по bootloader использовать контекст:

```text
Компонент: luna-boot.efi

Цель:
реализовать минимальный UEFI bootloader Project Luna.

Он должен:
- запускаться напрямую через UEFI;
- перехватывать B;
- показывать Boot Menu только при необходимости;
- читать System Image manifests;
- обнаруживать System Images;
- определять compatible kernels;
- выбирать current;
- поддерживать factory;
- выполнять soft fallback;
- поддерживать recovery;
- загружать выбранное kernel/System Image.

System Images находятся в:
system/images/

Kernels находятся в:
system/kernels/

Каждый System Image:
luna-X.Y.Z.squashfs
является непосредственно SquashFS.

Каждый System Image имеет рядом:
luna-X.Y.Z.toml

Boot state не должен переписываться на каждом boot.

Не заниматься:
- application management;
- .lbp;
- desktop;
- user data;
- service manager.

Главный вопрос чата:
[КОНКРЕТНАЯ ЗАДАЧА]
```

---

# 75. Отдельный чат: Bundle Format

Контекст:

```text
Компонент: luna-bundle

Цель:
спроектировать и реализовать Bundle Format v1.

Формат:
.lbp

Важно:
.lbp НЕ является System Image.

System Image:
.squashfs = SquashFS System Image

Bundle:
.lbp = отдельный Bundle Format.

RFC:
RFC-0002 — Bundle Format v1

Нужно определить:
- bundle structure;
- metadata;
- manifest;
- payload;
- dependencies;
- architecture;
- versions;
- installation;
- update;
- removal;
- integrity;
- compatibility.

Не заниматься:
- bootloader;
- kernel selection;
- System Image format;
- desktop.

Главный вопрос чата:
[КОНКРЕТНАЯ ЗАДАЧА]
```

---

# 76. Отдельный чат: System Images

Контекст:

```text
Компонент: System Image subsystem

Главный инвариант:

System Image = непосредственно SquashFS image.

Например:

system/images/luna-3.0.0.squashfs

Каждый image имеет отдельный manifest рядом:

system/images/luna-3.0.0.toml

Manifest должен описывать как минимум:
- version;
- architecture;
- format;
- compatible kernels;
- boot-related metadata.

System Images:
- versioned;
- immutable;
- stored in system;
- independent from data;
- independently updateable from kernels.

Нужно спроектировать:
- exact manifest;
- image validation;
- installation;
- deletion;
- retention;
- compatibility;
- version comparison;
- verification;
- hybrid loading.

Главный вопрос чата:
[КОНКРЕТНАЯ ЗАДАЧА]
```

---

# 77. Отдельный чат: Kernel subsystem

Контекст:

```text
Компонент: Kernel Management

Kernels находятся в system.

System Image и kernel обновляются независимо.

Каждый System Image manifest содержит информацию о compatible kernels.

Bootloader не должен показывать пользователю несовместимые kernels.

Fallback должен использовать только compatible kernels.

Нужно спроектировать:
- kernel metadata;
- versioning;
- compatibility;
- installation;
- removal;
- selection;
- fallback;
- kernel verification.

Главный вопрос:
[КОНКРЕТНАЯ ЗАДАЧА]
```

---

# 78. Отдельный чат: luna-fs

Контекст:

```text
Компонент: luna-fs

Цель:
filesystem/storage abstraction Luna.

Контекст:
- system является OS-managed;
- data является mutable;
- applications используют isolated mount namespaces;
- внешние устройства должны автоматически монтироваться;
- USB должен появляться в desktop без ручного mount command.

Нужно исследовать:
- mount namespaces;
- filesystem detection;
- automount;
- removable media;
- safe unmount;
- application filesystem views.

Не переносить System Images в data.

Главный вопрос:
[КОНКРЕТНАЯ ЗАДАЧА]
```

---

# 79. Отдельный чат: Application Runtime

Контекст:

```text
Компонент: Application Runtime

Цель:
запускать приложения из bundles в изолированном filesystem environment.

Главная идея:
mount namespaces.

Приложение должно видеть только разрешённую ему файловую среду.

Нужно отдельно спроектировать:
- filesystem namespace;
- permissions;
- user files;
- file picker;
- external devices;
- IPC;
- application data;
- shared resources.

Не создавать Docker-подобную ОС.

Использовать существующие Linux technologies.

Главный вопрос:
[КОНКРЕТНАЯ ЗАДАЧА]
```

---

# 80. Отдельный чат: Device Manager

Контекст:

```text
Компонент: Device Management

UX requirement:

USB/external storage подключается
→ устройство автоматически обнаруживается
→ filesystem определяется
→ volume автоматически монтируется
→ появляется в file manager.

Пользователь не должен вручную выполнять:
mount /dev/... /mnt/...

Нужно определить:
- backend;
- device discovery;
- filesystem detection;
- automount;
- permissions;
- safe removal;
- desktop integration.

Главный вопрос:
[КОНКРЕТНАЯ ЗАДАЧА]
```

---

# 81. Отдельный чат: Services

Контекст:

```text
Компонент: Service Management

Цель:
использовать существующую Linux service/init technology.

Направление:
OpenRC-like.

Не писать собственную service manager систему без необходимости.

Нужно определить:
- boot integration;
- service lifecycle;
- dependencies;
- logging;
- failure handling;
- interaction with system images.

Главный вопрос:
[КОНКРЕТНАЯ ЗАДАЧА]
```

---

# 82. Отдельный чат: luna-config

Контекст:

```text
Компонент: Configuration

Основной формат:
TOML там, где это подходит.

Нужно определить:
- system configuration;
- retention policy;
- user settings;
- boot configuration;
- configuration lifecycle;
- immutable vs mutable configuration.

Не смешивать конфигурацию с System Image payload.

Главный вопрос:
[КОНКРЕТНАЯ ЗАДАЧА]
```

---

# 83. Отдельный чат: luna CLI

Контекст:

```text
Компонент: luna CLI

Цель:
единый пользовательский интерфейс управления системой.

Направления:
- system;
- bundles;
- kernels;
- drivers;
- devices;
- recovery;
- configuration.

Точный CLI пока НЕ утверждён.

Нужно сначала спроектировать:
- command tree;
- permissions;
- output format;
- error handling;
- machine-readable mode;
- interaction with services.

Не придумывать команды как окончательную архитектуру без обсуждения.
```

---

# 84. Отдельный чат: Repository / Rust architecture

Контекст:

```text
Project Luna — Rust workspace.

Build:
cargo build

Нужно сохранять modular architecture.

Crate должен иметь одну понятную ответственность.

Не превращать luna-common в dumping ground.

Новые crates добавлять только если они создают реальную архитектурную границу.
```

---

# 85. Порядок дальнейшей разработки

С учётом всех решений разумный порядок:

## Этап 1 — Architecture baseline

Зафиксировать:

* directory layout;
* system/data boundary;
* boot architecture;
* System Image architecture;
* kernel architecture;
* bundle architecture.

## Этап 2 — RFC-0001

Оформить основной архитектурный RFC.

Он должен описывать фундамент Luna:

```text
purpose
principles
disk layout
boot architecture
system/data separation
immutable model
versioned images
kernel separation
```

## Этап 3 — RFC-0002

```text
Bundle Format v1
```

Спроектировать `.lbp`.

## Этап 4 — System Image Specification

Отдельно:

```text
SquashFS
+
per-image manifest
+
versioning
+
compatibility
+
retention
```

## Этап 5 — Boot specification

Описать:

```text
UEFI
 ↓
luna-boot.efi
 ↓
current
 ↓
manifest
 ↓
compatible kernel
 ↓
System Image
 ↓
fallback
 ↓
factory
```

## Этап 6 — Prototype

После спецификаций начать реализацию.

---


# 88. Правило для будущего AI-чата

Если новый чат получает этот документ, он должен действовать следующим образом.

## Шаг 1

Определить:

```text
Какой компонент обсуждается?
```

## Шаг 2

Прочитать соответствующий раздел.

## Шаг 3

Отделить:

```text
ПРИНЯТО
```

от:

```text
ОБСУЖДАЕТСЯ
```

## Шаг 4

Не изменять принятые решения молча.

## Шаг 5

Если новая идея конфликтует с архитектурой:

```text
ARCHITECTURE CONFLICT
```

и объяснить:

1. какое решение уже существует;
2. чем новая идея ему противоречит;
3. что изменится;
4. какие компоненты затронет;
5. почему изменение может быть полезно или вредно.

## Шаг 6

Только после архитектурного решения менять Source of Truth.

---

# 89. Главный антигаллюцинационный принцип

Если информации нет в Source of Truth:

> НЕ ВЫДУМЫВАТЬ, ЧТО МЫ ЭТО УЖЕ РЕШИЛИ.

Нужно сказать:

```text
Это ещё не определено.
```

или:

```text
Это обсуждалось, но окончательного решения нет.
```

или:

```text
Это моё новое предложение, а не принятое решение.
```

Это особенно важно для:

* форматов;
* файлов;
* manifest;
* boot state;
* CLI;
* kernel compatibility;
* security;
* permissions;
* runtime;
* update protocol.

---

# 90. Приоритет источников

Если несколько источников противоречат друг другу:

```text
1. Последнее явно принятое пользователем решение
2. Этот Source of Truth
3. Более ранние обсуждения
4. Предложения ассистента
5. Новые предположения
```

Особенно важно:

> предложение ассистента никогда не должно автоматически становиться решением проекта.

---

# 91. Что является настоящим решением, а что было предложением

Примеры ПРИНЯТЫХ решений:

```text
Project Luna
Rust
Apache 2
EFI/system/data/swap
custom luna-boot.efi
B → Boot Menu
System Images в system
kernels в system
System Image = SquashFS
manifest для каждого System Image
current
factory
soft fallback
compatible kernels
.lbр для bundles
applications в data/system/apps
drivers в data/system/drivers
users/data/cache в data
niri + Noctalia
Ghostty + fish
mount namespaces
automatic external device mounting UX
```

Примеры того, что ещё НЕ окончательно определено:

```text
точный .lbp формат
точный System Image manifest
точный kernel manifest
точный boot state format
точная hybrid RAM loading implementation
точный application permission model
точный automount backend
точный service backend
точный CLI
точная update transaction system
```

---

# 92. Каноническая схема Project Luna

```text
                         PROJECT LUNA
                              │
                 ┌────────────┴────────────┐
                 │                         │
             IMMUTABLE                  MUTABLE
               SYSTEM                     DATA
                 │                         │
        ┌────────┼────────┐          ┌─────┴─────┐
        │        │        │          │     │     │
      Images  Kernels  Metadata     Apps Users Data Cache
        │        │        │          │
        │        │        │        Drivers
        │        │        │
        └────────┼────────┘
                 │
            luna-boot.efi
                 │
                 ▼
           Linux kernel
                 │
                 ▼
          Luna System
                 │
                 ▼
        Application Runtime
                 │
          mount namespaces
                 │
                 ▼
          User Environment
                 │
        ┌────────┼────────┐
        │        │        │
       niri   Noctalia  Applications
                         │
                    Ghostty/fish
```

---

# 93. Каноническая boot схема

```text
                         UEFI
                           │
                           ▼
                    luna-boot.efi
                           │
                     B pressed?
                      /       \
                    no         yes
                    │           │
                    │       Boot Menu
                    │           │
                    │      select image
                    │           │
                    │           ▼
                    │      read manifest
                    │           │
                    └──────┬────┘
                           │
                           ▼
                        current
                           │
                           ▼
                  compatible kernels
                           │
                           ▼
                    select kernel
                           │
                           ▼
                  Linux kernel
                           │
                           ▼
                  Luna System Image
                           │
                     ┌─────┴─────┐
                     │           │
                  success      failure
                     │           │
                     │      compatible fallback
                     │           │
                     │           ▼
                     │      older kernel/image
                     │           │
                     │      ┌────┴────┐
                     │      │         │
                     │   success    failure
                     │      │         │
                     └──────┘         ▼
                                  factory
```

---

# 94. Каноническая модель обновления

Система:

```text
current
    ↓
new System Image
    ↓
validate
    ↓
install
    ↓
select compatible kernel
    ↓
boot
```

Kernel:

```text
current kernel
    ↓
new kernel
    ↓
validate
    ↓
check compatibility
    ↓
boot
```

Они не должны требовать обязательного совместного обновления.

---

# 95. Каноническая модель восстановления

```text
current
   │
   ├── working → continue
   │
   └── failed
          │
          ▼
   compatible kernel
          │
      ┌───┴───┐
      │       │
    works   fails
      │       │
      │       ▼
      │   another compatible
      │       │
      │       ▼
      │   another image
      │       │
      │       ▼
      │     ...
      │
      └──────────────► system

If everything fails:

current
  ↓
fallbacks exhausted
  ↓
factory
```

---

# 96. Самое главное архитектурное разделение

Весь проект можно держать в голове через четыре слоя:

```text
EFI
 ↓
BOOT
 ↓
SYSTEM
 ↓
DATA
```

Где:

```text
EFI
```

запускает.

```text
BOOT
```

решает, что запускать.

```text
SYSTEM
```

содержит версии самой ОС и kernels.

```text
DATA
```

содержит всё изменяемое.

Это должно быть одной из главных архитектурных идей Luna.

---

# 97. Текущее состояние проекта

На данный момент:

### Уже сделано

```text
[done]
Project Luna repository
[done]
Apache 2 license
[done]
Rust workspace
[done]
Cargo workspace
[done]
luna
[done]
luna-common
[done]
luna-log
[done]
luna-fs
[done]
luna-bundle
[done]
luna-config
[done]
successful cargo build
```

### Архитектурно принято

```text
[accepted]
minimal immutable system
[accepted]
EFI/system/data/swap
[accepted]
custom luna-boot.efi
[accepted]
[accepted]
B boot menu
[accepted]
System Images in system
[accepted]
SquashFS System Images
[accepted]
one manifest per System Image
[accepted]
kernels in system
[accepted]
current
[accepted]
factory
[accepted]
kernel compatibility
[accepted]
soft fallback
[accepted]
.lbp Bundle Format
[accepted]
applications as bundles
[accepted]
isolated dependencies
[accepted]
mount namespaces
[accepted]
automatic external storage mounting UX
[accepted]
niri + Noctalia
[accepted]
Ghostty + fish
```

### Нужно разработать

```text
[next]
RFC-0001
[next]
RFC-0002 Bundle Format v1
[next]
System Image specification
[next]
System Image manifest
[next]
kernel metadata
[next]
boot state
[next]
luna-boot prototype
[next]
bundle prototype
[next]
application runtime
[next]
device manager
[next]
service integration
[next]
luna CLI
```

---

# 98. Главная задача следующих этапов

Не начинать писать огромное количество кода прямо сейчас.

Сначала нужно сделать архитектуру достаточно точной, чтобы код уже реализовывал договорённость.

Приоритет:

```text
Architecture
    ↓
RFC
    ↓
Format
    ↓
Interfaces
    ↓
Prototype
    ↓
Implementation
    ↓
Integration
```

а не:

```text
Code
 ↓
Code
 ↓
Code
 ↓
потом выяснить, что два компонента используют разные архитектуры
```

---

# 99. Правило разработки одного компонента

Когда начинаем отдельный компонент:

1. Сначала определить его ответственность.
2. Определить его границы.
3. Определить входы.
4. Определить выходы.
5. Определить зависимости.
6. Определить persistent state.
7. Определить API.
8. Определить ошибки.
9. Только потом писать код.
10. После реализации написать тесты.
11. Проверить интеграцию.
12. Не захватывать ответственность другого компонента.

---

# 100. Правило для Rust crates

Каждый crate должен отвечать на вопрос:

> "Зачем этот crate существует?"

Если ответ:

```text
"там просто всякие общие вещи"
```

то архитектура, скорее всего, плохая.

Например:

```text
luna-bundle
```

отвечает за bundles.

```text
luna-fs
```

отвечает за filesystem.

```text
luna-config
```

отвечает за configuration.

```text
luna-log
```

отвечает за logging.

```text
luna-common
```

содержит только действительно общие фундаментальные структуры.

---

# 101. Финальная карта проекта

```text
Project Luna
│
├── Documentation
│   ├── CHARTER.md
│   ├── RFC-0001
│   ├── RFC-0002
│   └── System Image Specification
│
├── Boot
│   └── luna-boot.efi
│
├── System
│   ├── images
│   │   ├── System Images (SquashFS)
│   │   └── per-image manifests (.toml)
│   └── kernels
│
├── Bundles
│   └── .lbp
│
├── Filesystem
│   ├── mounts
│   ├── namespaces
│   └── devices
│
├── Runtime
│   ├── application isolation
│   ├── permissions
│   └── application data
│
├── Devices
│   └── automount
│
├── Services
│   └── OpenRC-like
│
├── CLI
│   └── luna
│
├── Configuration
│   └── TOML
│
└── Desktop
    ├── niri
    ├── Noctalia Shell
    ├── Ghostty
    └── fish
```

---

# 102. Самая короткая формула Luna

Если весь проект нужно объяснить в нескольких строках:

```text
Project Luna is an operating system built around the Linux kernel.
very small immutable foundation.

The OS is divided into EFI, system, data and swap.

EFI contains luna-boot.efi.

system contains versioned Linux kernels and versioned Luna System
Images.

A System Image is directly a SquashFS image and has its own
manifest.

current identifies the current image/kernel combination.

factory identifies the original factory-good image/kernel
combination.

Boot uses manifest-defined kernel compatibility and supports
soft fallback.

data contains applications, drivers, users, mutable data and
cache.

Applications use a bundle model (.lbp) and isolated filesystem
namespaces.

The system is managed through the luna tool.
```

---

# 103. Главный принцип для всех будущих обсуждений

**Не путать "мы обсуждали" и "мы решили".**

В Project Luna это особенно важно.

Если появилась новая идея, её нужно сначала обозначить:

```text
Proposal
```

После обсуждения:

```text
Accepted
```

После принятия она становится частью Source of Truth.

Если решение отменено:

```text
Superseded
```

и нужно явно указать, каким решением оно заменено.

Таким образом, история архитектуры остаётся понятной и не превращается в набор противоречащих друг другу сообщений.

---

# 104. Текущая архитектурная точка, с которой продолжаем

На данный момент следующая логическая точка старта:

```text
Project Luna
      │
      ├── Architecture baseline
      │
      ├── RFC-0001
      │
      ├── RFC-0002
      │      └── Bundle Format v1 (.lbp)
      │
      ├── System Image Specification
      │      ├── SquashFS
      │      └── per-image manifest
      │
      ├── Kernel Specification
      │
      └── Boot Specification
             └── luna-boot.efi
```

И только после того, как эти границы достаточно хорошо определены, мы начинаем активно расширять код.

---

# 105. Current architecture after Phase 1.1–1.4

This section is the current consolidated Source of Truth. It supersedes earlier sections where explicitly stated below. Phase working documents are historical/traceability material; they are not independent architectural authorities.

## 105.1 Canonical storage model

The installed machine has four physical areas:

```text
EFI      — bootloader storage; hidden from ordinary users
SYSTEM   — immutable/versioned OS images and kernels; hidden from ordinary users
DATA     — mutable user-visible storage
SWAP     — optional swap policy; may be absent, a swap file/partition, and/or ZRAM
```

The ordinary user sees DATA as:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

There must not be duplicate `data/apps`, `data/users`, `portable`, or similar parallel trees merely for organizational purposes.

`DATA/system` is the OS-managed mutable system area. Applications are installed there as bundles; drivers, managed libraries/dependencies, and managed external-volume representations live there as well.

The file manager may expose dedicated **Apps** and **Volumes** views. Apps is backed by `DATA/system/apps`; Volumes is backed by `DATA/system/volumes` and presents friendly volume names rather than raw `/dev` paths.

## 105.2 Users and mutable state

Every local user has exactly three top-level directories:

```text
DATA/users/<user>/
├── home/
├── data/
└── config/
```

- `home/` contains ordinary user folders such as Documents and Downloads.
- `data/` contains mutable application data belonging to that user.
- `config/` contains user/application configuration.

The normal logical application home is `/home/<user>/`. Access to another user's home is not granted by default.

A normal interactive runtime is represented by a `UserSession` entity. User identity and session state are intentionally combined at this architectural layer; Luna does not model the normal desktop as a Linux-style collection of independent TTY sessions.

The runtime hierarchy is:

```text
luna-system-runtime
├── UserSession A
│   ├── app-runtime
│   │   ├── ApplicationInstance
│   │   └── ApplicationInstance
│   └── GUI/Desktop session
│
└── UserSession B
    ├── app-runtime
    │   ├── ApplicationInstance
    │   └── ApplicationInstance
    └── GUI/Desktop session
```

Each `UserSession` combines one user identity with its session state and session-scoped policy. User/session behaviour may independently be configured as ACTIVE/continue, RESTRICTED, or TERMINATED. The default behaviour when leaving a user session is RESTRICTED.

The normal desktop startup is a single system-managed graphical session path: the user reaches the Luna graphical login/welcome environment directly rather than starting a TTY and manually launching a Wayland desktop. Additional session types may be introduced later, but the core desktop model does not depend on Linux-style TTY sessions.

System services are not tied to a single `UserSession`. An update transaction may continue while one UserSession becomes restricted or terminates and another UserSession becomes active.

## 105.3 Logical root and hybrid loading

Luna does not physically reproduce the traditional Linux root directory tree inside DATA.

At boot, Luna first establishes a minimal RAM/virtual logical root. Only after this foundation exists does it attempt to attach DATA. This ordering deliberately allows the system to enter Recovery when DATA is unavailable without first depending on normal persistent user DATA.

The logical root is a conventional Linux-compatible `/` from the kernel/application point of view, but its physical implementation is a hybrid composition of RAM/virtual filesystem state, immutable System Image content and controlled DATA mappings.

System Images are SquashFS and may be loaded lazily as required. The exact low-level implementation remains an implementation/specification task, but the hybrid/lazy model is accepted.

## 105.4 `luna-root-mapping`

The component formerly referred to as `luna-root` is conceptually named `luna-root-mapping` for precision.

Its responsibility is narrow:

- construct the logical Linux-compatible root;
- compose controlled physical-to-logical mappings;
- enforce mapping classes/allowed relationships together with the permission layer;
- maintain the namespace-local mapping state required by the runtime.

It must not absorb application installation/update/removal, application lifecycle, session management, recovery, or general system management.

The implementation direction is a small daemon plus a library. Mapping rules are kept as lightweight rules/configuration rather than duplicated in every process.

## 105.5 Mapping model

Mappings are file-oriented, policy-controlled composition rules. Whole-directory blind mapping is not the default model.

There is no single global mapping table for all applications. Each application namespace receives a small table containing only the mappings it requires. Mapping state is primarily held in RAM.

Conceptual lookup precedence depends on the semantic mapping class. For configuration and mutable application state, the accepted precedence is:

```text
user DATA / override
        ↓ if absent
application-provided/default content
        ↓ if absent
system DATA / system configuration
        ↓ if absent
System Image default
```

This is not a universal fallback chain for every logical path. Each mapping class defines its permitted source order. An explicit deny stops resolution rather than falling through to a lower layer.

Mapping rules are class-restricted. A configuration class such as logical `/etc` may resolve into user configuration, but an arbitrary user path such as `/users/<user>/config/bin` must not automatically become an executable/system path. The mapping policy defines which physical resource classes are allowed to satisfy which logical path classes.

This same policy model is used together with permissions: visibility, readability and writability are distinct states.

## 105.6 Application filesystem view

Every application gets its own Linux-compatible logical filesystem view and its own mount/filesystem namespace.

The application should experience the namespace as a clean system in which it is the owner of its own application environment, while the actual physical resources are selectively composed underneath it.

The namespace may expose:

- the application's own bundle;
- required shared libraries/dependencies;
- required System Image files;
- the current user's permitted files/configuration/data;
- explicitly permitted external volumes/devices/resources.

Files belonging to unrelated applications or other users are not visible merely because they exist on DATA.

Different applications may map the same logical path to different dependency versions, for example:

```text
App A: /libs/gtk → DATA/system/libs/gtk/3
App B: /libs/gtk → DATA/system/libs/gtk/4
```

The application sees the same logical `/libs/gtk` path in both cases.

## 105.7 Permission and security model

Permissions are a separate architectural layer rather than an incidental feature of the runtime or application manager.

The security/policy subsystem is the central policy authority. Kernel/filesystem primitives and Linux namespace mechanisms still physically enforce the restrictions.

The model is capability/permission-oriented and can be extended with multiple levels rather than a binary allow/deny. Applications do not receive unrestricted filesystem, device, volume or user-data access by default.

Permission policy can be shared by all instances of the same application identity/version where appropriate. Launching a second instance must not require duplicating identical permission state when the policy is inherently application-wide.

The architecture intentionally leaves room for stronger security mechanisms in the future.

## 105.8 Application bundles

An installed application is an immutable bundle. The bundle is independent from its mutable user data.

`.lbp` is the transport/archive Bundle Format; it is not a System Image format. System Images remain direct SquashFS images.

A bundle may be physically moved to another location or removable drive without moving its mutable user state. Applications outside `DATA/system/apps` can be launched directly from the file manager. `DATA/system/apps` is the installed-app registry/view, not a requirement that every executable must physically remain there forever.

The file manager may register externally stored applications through a link/registration mechanism so they still appear in the Apps view.

Multiple versions of the same application may coexist and run independently when their identities/versions permit it. A newer version must not destroy the ability to run an older working version merely because an update exists.

## 105.9 Application Manager and Application Runtime

The application manager is `luna-app-manager`.

It is responsible for application lifecycle operations such as:

- installation;
- update;
- removal;
- verification/integrity checks;
- compatibility checks;
- migrations associated with updates;
- orphaned application-data discovery and cleanup policy;
- package ingestion, including future support for `.deb` and `.rpm` by converting/installing them into Luna bundles and generating the required manifest metadata.

`luna-app-manager` does **not** own application execution.

Application execution is owned by `luna-app-runtime` together with the system runtime. `luna-app-manager` does not own application launch; it manages application artifacts and their lifecycle outside runtime execution. The launch chain is therefore conceptually:

```text
User / GUI / CLI
        ↓
luna-system-runtime
        ↓
UserSession
        ↓
luna-app-runtime
        ↓
ApplicationInstance
```

The exact IPC/API boundary remains to be specified.

`luna-app-runtime` is responsible for constructing the application execution environment, namespace, mappings, permissions and lifecycle state. `luna-system-runtime` is the single system-level runtime coordinator and owns the `UserSession` hierarchy; each `UserSession` owns one `luna-app-runtime` context. A separate supervisor component is not required initially; it may be introduced later if justified.

`ApplicationInstance` is the runtime-level representation of one running instance. The architecture supports asynchronous, multithreaded and multicore execution.

## 105.10 Application lifecycle and namespace state

By default, closing an application releases its runtime memory/resources. Persistent in-memory retention after close may be exposed as an application/user setting, but it is not the default.

Mapping tables and related runtime state may remain in RAM after an application exits for faster relaunch, but retention is configurable and adaptive. The system may evict older retained state under memory pressure, starting with the oldest/least recently used state until enough RAM is available.

Runtime state is not required to be persisted to disk merely to survive an application restart.

Multiple instances of the same application may share identical mapping/permission state when policy is application-wide, avoiding unnecessary duplication in RAM.

Application single-instance/multi-instance behavior is a combination of application metadata and user preference. Different versions of an application are independently launchable when compatible.

## 105.11 Application data lifecycle

Moving or removing an application bundle does not automatically imply immediate deletion of its mutable user data.

`luna-app-manager` can detect data that is no longer associated with an installed application and expose it for deliberate cleanup.

Automatic cleanup is governed by retention rules. Data may be protected from automatic cleanup and require explicit user action. Manual application removal may ask whether application data should also be removed.

Application data and configuration are user-owned mutable state; the immutable bundle itself remains the protected executable unit.

## 105.12 User-visible application modifications

Installed bundles are immutable, but Luna intentionally allows controlled user customization of application metadata/configuration where the architecture permits it.

A user may inspect an installed bundle and, for example, correct compatibility metadata that is unnecessarily restrictive. Such changes must not directly mutate the immutable bundle payload. Instead, the system may provide a controlled override/hook mechanism in user data, validated by the policy layer.

This allows compatibility overrides without turning immutable application payloads into arbitrary writable software trees.

## 105.13 External volumes and devices

External disks and USB media are automatically detected and exposed through a friendly volume model.

Internally, managed volume state is represented under:

```text
DATA/system/volumes/<volume-name-or-id>/
```

The file manager exposes a dedicated Volumes view. A connected flash drive should be usable without manual `mount` commands.

Read/write behavior may be configurable. Automatic execution from removable media is disabled by default or requires an explicit user/system policy; connecting a hostile USB device must not silently execute software.

Device access is permission-controlled just like filesystem access.

## 105.14 System images and kernels

SYSTEM contains versioned System Images and kernels independently:

```text
SYSTEM/
├── images/
│   ├── <version>.squashfs
│   ├── <version>.toml
│   └── ...
└── kernels/
    ├── <kernel>
    └── ...
```

Each System Image is directly a SquashFS image with its own manifest. System Images and kernels have independent lifecycle/compatibility metadata.

`current` identifies the current image/kernel combination. Separate persistent boot-state metadata is maintained independently of the System Image data so the bootloader can change boot decisions without requiring normal userspace access.

Compatibility is a model/query responsibility of the System Image and kernel management layers. A kernel is selected only when compatible with the chosen image.

## 105.15 Factory state

Factory is a pair of immutable original installation entities:

- Factory System Image;
- Factory Kernel.

They are the known-good state written at OS installation. They are not ordinary retention candidates and must never be deleted or modified by normal update/cleanup operations.

Factory is not merely another recent checkpoint. It is the original guaranteed-good installation state.

## 105.16 Boot and fallback

The boot path remains:

```text
UEFI
  ↓
luna-boot.efi
  ↓
boot-state / current
  ↓
compatible kernel
  ↓
compatible System Image
  ↓
logical root
  ↓
DATA attach
  ↓
normal user sessions
```

Normal boot is direct. Pressing `B` during startup opens the boot/recovery menu.

Fallback is compatibility-aware. Conceptually, if the newest image fails, Luna tries an older compatible image with the current kernel when possible. If no compatible image remains for that kernel, the bootloader may move to the previous compatible kernel and try the compatible image set for it. If all normal combinations fail, Factory is selected.

A System Image failure may support soft fallback without a full machine reboot where technically safe. A kernel panic requires reboot before selecting another kernel.

Boot-state changes are event-driven rather than rewritten on every ordinary boot.

## 105.17 Recovery

Recovery is a fully working repair environment, not merely a read-only shell and not Factory.

The intended model is:

```text
minimal RAM root
     ↓
Recovery System Image
     ↓
virtual recovery user
     ↓
repair / diagnostics / recovery tools
```

Recovery does not depend on normal persistent DATA. Its writable state is RAM-backed and disappears on reboot.

Recovery can diagnose, repair or remove broken DATA components such as incompatible drivers. It may expose specialized repair tools and a minimal set of broadly usable drivers.

Recovery is deliberately useful even when normal DATA is unavailable.

Recovery must not automatically grant unrestricted access to every user's protected data. User DATA is protected by the security model and may require authentication/authorization before being opened. The exact recovery authentication UX remains a detailed security-design task.

If the normal System Image itself cannot start, Factory remains the final known-good path.

## 105.18 Updates and ownership boundaries

`luna-system-manager` owns the system state model/query layer. It describes what the system currently is and what combinations/states are valid.

`luna-update-manager` is the executor of changes. It owns installation/update/removal transactions for System Images and kernels, including image assembly/update mechanics and retention actions.

`luna-kernel-manager` owns kernel model/query/compatibility information and kernel state, but kernel installation/update/removal is executed by `luna-update-manager`.

The same separation applies to applications: `luna-app-manager` owns application management, while `luna-app-runtime` owns execution.

The updater may continue a transaction while users switch sessions.

System protection is layered: System Images are immutable/read-only in normal operation, and the updater has the dedicated authority required to perform controlled changes. This prevents ordinary processes from deleting the currently running image.

## 105.19 Runtime resource protection

The system reserves resources for itself so a misbehaving application cannot consume all available resources and make the OS unresponsive.

Linux mechanisms are used initially for resource reservation/isolation rather than inventing a new kernel resource controller.

The same principle applies conceptually to CPU, memory and GPU resources. Exact quotas and enforcement policy remain implementation work.

Memory compression/swap policy supports optional disk swap and ZRAM. Installation and settings can expose the policy; ZRAM can provide swap-like capacity without persistent disk storage.

## 105.20 Sessions and user authority

There is no separate root user and no `sudo`/`su`-style privilege hierarchy as a required Luna architecture.

Users themselves have roles/permissions. An administrator can perform system/application management operations directly; a restricted/guest user receives only the permissions granted to that user.

The system must nevertheless support protected administrative operations even when a machine changes hands. An administrator password/system credential is accepted as an architectural direction: it can be used to restore administrative authority to a user or authorize privileged management operations without creating a permanent root account.

The exact credential storage, recovery and authentication protocol remain open security-specification work.

Users are isolated from one another by default. One user must not simply open another user's private data without authorization.

## 105.21 Checkpoints and rollback

Btrfs snapshots are used as a user-visible checkpoint/rollback subsystem, not as runtime state and not as a substitute for backups.

The feature is configurable. The user may choose between the accepted checkpoint scopes discussed in Phase 1.2 or disable the mechanism. The default is the narrower accepted option.

Snapshots are visible and manageable by the user. Exact scope, retention and transaction semantics remain to be specified.

## 105.22 CLI architecture

The main executable is `luna-cli`.

The CLI is a thin client over backend/service APIs. GUI and CLI should call the same backend rather than implement separate management logic.

Human-friendly short aliases are intentionally supported. Examples include concepts such as:

```text
app install <app>
app i <app>
app u <app>
app d <app>
sys update
sys -u
dev list
```

Exact command names are configurable through an alias system, including user-defined aliases. Settings should display a description of the underlying operation so users are not forced to remember long internal names.

The internal canonical component names remain explicit (`luna-app-manager`, `luna-update-manager`, etc.) even when the CLI presents short aliases.

## 105.23 Core component map after Phase 1.3

The architecture uses separate components where there is a real responsibility boundary. The current conceptual map is:

```text
luna-cli
│
├── luna-system-manager
├── luna-app-manager
├── luna-device-manager
├── luna-update-manager
├── luna-kernel-manager
├── luna-root-mapping
├── luna-security
├── luna-system-runtime
├── luna-app-runtime
├── luna-fs
├── luna-bundle
├── luna-config
├── luna-log
└── luna-common
```

This is an architectural map, not a statement that every crate/daemon already exists in the repository.

Manager components use a small daemon + library model where appropriate. The CLI and future GUI are thin clients over the same backend APIs.

`luna-common` must remain small and must not become a dumping ground.

`luna-fs` is a low-level filesystem abstraction crate.

`luna-bundle` owns the internal bundle representation/format concerns; lifecycle operations belong to `luna-app-manager`.

`luna-config` owns configuration concerns and should be redesigned against the current architecture rather than blindly preserving early placeholder APIs.

## 105.24 System runtime model

There is one system runtime supervising system-level runtime state and the app runtimes of multiple users.

Conceptually:

```text
luna-system-runtime
    ├── app-runtime(user A)
    │     ├── ApplicationInstance
    │     └── ApplicationInstance
    │
    └── app-runtime(user B)
          └── ApplicationInstance
```

A single system runtime is preferred over one independent supervisor per user. If one application runtime fails, the common system runtime can detect, diagnose and notify the affected user without requiring the other user's runtime to fail.

Application execution is asynchronous and designed for modern multicore/multithreaded systems.

## 105.25 Event and state model

The architecture prefers explicit persistent state where it is the source of truth, with event-driven changes rather than unnecessary rewrites.

State that must be shared consistently across users/applications is system state, not per-process duplicated state. Event/log streams may use a Kafka-like conceptual model where useful, but the implementation may use lighter mechanisms when appropriate.

The architecture favors asynchronous operation for system responsiveness.

## 105.26 Shared libraries and dependency management

Shared libraries are stored under `DATA/system/libs` and can be reused by multiple applications where compatible, rather than duplicated into every bundle.

Namespace-local mapping isolates incompatible versions without requiring separate physical copies of the entire application environment.

If a required dependency is absent, the system should identify what is missing and ask the user before downloading external content when user consent is appropriate. Luna should not silently pull arbitrary dependencies merely because an application requests them.

## 105.27 Resource cleanup and retention

The system owns memory cleanup globally rather than treating memory as belonging permanently to the currently logged-in user.

The system may reclaim retained runtime state, caches, namespace state and other reclaimable memory across users according to age, pressure and policy.

Retention policies should be configurable where user intent matters, with safe defaults.

## 105.28 Architecture conflict and supersession rules

The following rules are mandatory for future development:

1. Do not confuse “discussed” with “accepted”.
2. A new idea is `Proposal` until explicitly accepted.
3. An accepted decision becomes part of this Source of Truth.
4. A replacement is `Superseded` only when the replacement is explicit.
5. If implementation conflicts with an accepted decision, mark `ARCHITECTURE CONFLICT` before changing it.
6. Historical phase documents are evidence/traceability, not competing Sources of Truth.
7. The current `docs/ARCHITECTURE.md` is the single architectural Source of Truth.

## 105.29 Phase 1.4 accepted organizational rule

Every future architectural phase must update this file as decisions are accepted. Phase documents may preserve questions, reasoning and chronological answers, but they must not become a second Source of Truth.

When a phase closes, its accepted decisions are consolidated here and the phase document becomes historical/traceability material.

## 105.30 Current development status

```text
Phase 1.1  — accepted and consolidated
Phase 1.2  — accepted and consolidated
Phase 1.3  — accepted and consolidated
Phase 1.4  — in progress; accepted decisions A–T consolidated here
```

The project remains design-first. Architecture/interfaces/specifications precede substantial implementation.

# 106. Explicitly superseded statements from earlier sections

The following earlier statements in this document must no longer be treated as current when they conflict with section 105:

- the earlier DATA layout containing a top-level `data/` directory;
- any model that places System Images or kernels in DATA;
- any model that treats `luna-root` as a general-purpose manager rather than a root/mapping component;
- any model where `luna-app-manager` owns application execution;
- any model where permissions are merely an incidental runtime feature rather than a separate policy layer;
- any model where a single global mapping table serves all application namespaces;
- any model where Recovery is merely Factory or a read-only shell;
- any model where Factory can be deleted by normal retention/cleanup;
- any model that introduces a permanent root user or requires `sudo`/`su` as the privilege architecture;
- any early placeholder crate structure that conflicts with the Phase 1.3 responsibility map.

Historical text is retained above for traceability, but section 105 is the current interpretation.

# 107. Current architectural checkpoint

The current Source of Truth checkpoint is:

```text
Project Luna
│
├── EFI
│   └── luna-boot.efi
│
├── SYSTEM(Ext4)
│   ├── versioned System Images (SquashFS + per-image manifest)
│   ├── versioned kernels
│   ├── current boot-state
│   └── immutable factory image + factory kernel
│
├── DATA (Btrfs, user-visible)
│   ├── system/
│   │   ├── apps/
│   │   ├── drivers/
│   │   ├── libs/
│   │   └── volumes/
│   ├── users/<user>/{home,data,config}/
│   └── cache/
│
└── SWAP / ZRAM (policy-driven)
```

Logical execution is:

```text
minimal RAM logical root
        ↓
System Image / lazy SquashFS composition
        ↓
per-namespace root + file mappings
        ↓
permission policy
        ↓
app-runtime
        ↓
ApplicationInstance
```

Normal storage failure path is:

```text
Normal
  ↓ DATA unavailable
Recovery (RAM writable state + virtual recovery user)
  ↓ System Image unavailable
Factory (immutable original image + kernel)
```

The system is intentionally not a conventional Linux distribution layout. Linux kernel mechanisms are reused where useful, while Luna defines its own user-visible storage model, namespace composition, management boundaries and recovery architecture.

# 108. Phase 1.4 accepted decisions U–BT

Phase 1.4 extends the component and runtime architecture with application identity/versioning, package ingestion, trust, administrative authority, operations/events, system state, configuration state and resource-management semantics.

## 108.1 Application identity and versions

Applications have separate identities for the application, concrete bundle/build, version, running instance and user context.

```text
ApplicationID
BundleID
Version
ApplicationInstanceID
UserID
```

`ApplicationID` is the stable identity of an application. Different versions remain independently installable and runnable when otherwise compatible. A single-instance restriction applies to a specific application version/bundle identity; different versions may therefore run concurrently.

Default application selection is layered:

```text
system default
    ↓
user override
    ↓
explicit version selection
```

An application may declare whether multiple instances are supported, while user preference determines the preferred launch behaviour where multiple instances are permitted.

## 108.2 Application data migration

Application updates may require migration of persistent user data.

`luna-app-manager` orchestrates migrations, while the application provides the migration logic. A checkpoint is mandatory before migrations that can change persistent data irreversibly.

For the initial model, migration is performed when required rather than maintaining multiple backward data-format paths. If a required migration fails, the update must stop and the protected checkpoint must be available for rollback. The user must be warned rather than silently losing or transforming data.

## 108.3 Bundle formats and package ingestion

An installed application remains an immutable Luna Bundle.

`.lbp` is the transport/archive representation of a Bundle and is not the runtime representation.

The application manager may ingest external package formats such as `.deb` and `.rpm`. These are treated as input formats and are converted into Luna Bundle representations with Luna manifest metadata. Imported packages are handled on a best-effort compatibility basis; Luna does not promise that arbitrary package scripts or assumptions about a conventional Linux filesystem will always work.

Package installation scripts are analyzed in a restricted import environment. Supported behaviours may be translated into Luna-native operations; unsupported behaviours must be reported rather than silently granted unrestricted system access.

## 108.4 Bundle immutability and controlled overrides

Installed Bundle payloads are immutable.

Compatibility and metadata customizations are represented as controlled user overrides/hooks rather than direct mutation of the immutable Bundle payload.

Overrides are user-visible through the GUI and may also be exposed to advanced users. Only fields explicitly marked as overrideable may be changed.

A modified or overridden Bundle enters a non-original trust state. The user must receive a warning and may choose to cancel, launch once, or create an explicit local trust record. Local trust must never be created silently.

## 108.5 Permissions and capabilities

`luna-security` is the central permission/capability policy authority.

The effective policy combines:

```text
system policy
    ↓
user policy
    ↓
application policy
    ↓
instance constraints
```

A lower layer may further restrict an upper layer but may not weaken an enforced denial.

Applications declare requested capabilities in their manifests; declarations are requests and do not grant access by themselves.

Permission decisions support multiple levels rather than a binary allow/deny model. The initial conceptual levels include denial, request/ask and allow, with more specific modes such as one-time or while-running access where appropriate.

Visibility, readability and writability remain separate policy dimensions.

## 108.6 Administrative authority

Luna has no permanent root user and does not require `sudo`/`su` as its privilege architecture.

Users have roles and capabilities. Administrative authority is a protected system capability.

At least one administrative recovery path must remain available. If a machine would otherwise lose its only administrator, the system must require a protected administrative credential before allowing the last administrator to be downgraded. An administrator credential may be used for operation-level elevation or a temporary administrative session.

An administrator is not automatically granted another user's private data. Access to another user's protected data requires appropriate authorization. Future encryption must allow user-data decryption authority to remain separate from system administration authority.

Administrator credentials must not be empty. Ordinary users may be passwordless where policy permits.

A future credential-recovery mechanism is part of the architecture and may use a recovery medium or externally stored recovery key/hash material. The exact authentication and cryptographic protocol remains open security-specification work.

## 108.7 Trust and signatures

Trust, signature validity and permission are separate concepts:

```text
signature validity
        ↓
trust state
        ↓
permission policy
```

A valid signature does not automatically grant execution permission. Supported trust states include factory/trusted, official, locally trusted, modified and untrusted states as required by the security model.

Signature infrastructure is an architectural extension point for System Images, kernels, Bundles and manifests.

## 108.8 Update channels and sources

System and application updates may be associated with channels such as stable, beta, nightly or local sources. The exact channel taxonomy remains configurable.

Update sources are represented conceptually as trusted sources. Sources may include official repositories, third-party repositories, local files, removable media and future network/share sources. Verification and trust policy are applied before installation.

Offline installation from local `.lbp` or other supported update artifacts is supported.

## 108.9 Operations

Long-running asynchronous actions are represented as `Operation` objects rather than requiring the user interface to remain attached to a process.

An operation conceptually contains:

```text
Operation
├── id
├── type
├── state
├── progress
├── owner
├── start time
├── result
└── error
```

Operations are system-owned or jointly associated with the requesting user and the system. Permissions determine who may inspect or control an operation.

Cancellation is optional and stage-dependent: operations may advertise whether cancellation is supported, and irreversible stages may become non-cancellable.

## 108.10 Events, logs and diagnostics

Luna separates:

```text
Event
    = what happened

Log
    = detailed record of execution

Diagnostic report
    = system interpretation of cause/state and possible recovery
```

A system event model is part of the architecture. Event streams may use Kafka-like concepts such as producers, consumers, filtering and subscriptions without requiring Kafka itself.

Events may be ephemeral or persistent depending on their importance. Event audiences may distinguish system-wide events, user-specific events and application-specific events.

Diagnostics may subscribe to relevant event classes rather than receiving arbitrary unrelated event traffic.

## 108.11 System state and desired state

`luna-system-manager` owns the aggregate logical System State model.

System State and Boot State are separate:

```text
System State
    = what the running system knows about itself

Boot State
    = the persistent metadata needed by luna-boot.efi to make boot decisions independently of the running system
```

The architecture supports both `CurrentState` and `DesiredState`. The system manager owns the state model and query semantics; `luna-update-manager` executes transactions which move actual state toward the desired state.

Boot/recovery requests remain a separate concern from the aggregate runtime System State.

## 108.12 Health model

The system health model uses:

```text
Healthy
Degraded
Recovering
Failed
Emergency
```

`Emergency` is a health/diagnostic state, not a separate Boot Menu mode. The normal Boot Menu remains limited to normal boot, selection/alternative boot, Recovery and Factory paths as previously defined.

A failure in one user's app-runtime must not automatically fail other users' runtimes. The common `luna-system-runtime` detects, diagnoses and coordinates isolated recovery.

## 108.13 Runtime resource policy

Resource policy is hierarchical:

```text
System
  ↓
User
  ↓
Application
  ↓
ApplicationInstance
```

The system maintains protected resource reserve for its own operation. Linux resource-control mechanisms are preferred for enforcement where practical, including cgroup-based memory and CPU control. `memory.min`/`memory.low` can provide protection, while `memory.high` and `memory.max` provide throttling and hard limits; the exact values and policy remain subject to implementation and hardware constraints. citeturn102804search0

Resource policies may be adaptive. Free resources may be temporarily used by applications, but the system must preserve the minimum protected budget needed for system-runtime, diagnostics and critical services.

Memory pressure follows a general reclaim order from disposable caches and retained runtime state toward application pressure and controlled termination as a last resort. ZRAM and disk swap remain configurable options.

CPU and I/O resource policies are also planned to use available Linux mechanisms rather than replacing kernel scheduling/resource control. GPU policy is capability-based because support varies by hardware and driver.

## 108.14 Application resource sharing

Multiple ApplicationInstances of one application version may share identical mapping and permission policy where the policy is inherently application/user-wide.

Resource limits remain configurable at application and instance levels inside an application-wide and user-wide budget.

The default close behaviour is to terminate processes and release active runtime memory. Retained mapping/policy cache may survive in RAM for faster relaunch, with adaptive reclamation under memory pressure.

## 108.15 Configuration state

Configuration and runtime State remain distinct concepts.

Immutable defaults reside in the System Image; mutable user overrides reside in DATA and take precedence:

```text
DATA override
    ↓
System default
```

Deleting an override restores the System Image default.

Configuration changes never mutate an immutable System Image.

Configuration entries may declare application semantics such as live application, user-session restart, system-runtime restart or reboot requirement.

## 108.16 Volumes and removable media

`DATA/system/volumes` represents the system-managed volume view. Connected storage is exposed there using user-meaningful volume names and is also presented through the dedicated Volumes UI.

External applications may be launched directly from removable or external storage. Such applications receive the same runtime identity rules as equivalent installed Bundles, while untrusted external Bundles are subject to explicit warning/trust policy.

Removable-media autorun is independently configurable and must not silently execute arbitrary programs merely because a device was connected.

## 108.17 Recovery security

Recovery is a full working repair environment, not Factory and not a permanent root-user mode.

Recovery has:

```text
Recovery System Image
Recovery Kernel / compatible boot path
virtual temporary recovery user
RAM-backed writable runtime state
recovery diagnostics and repair tools
```

Recovery can operate without normal persistent DATA. User DATA is not automatically exposed merely because Recovery is running.

Protected user data is unlocked explicitly and only after suitable authentication/authorization. Recovery authority is therefore distinct from unrestricted access to every user's private data.

Recovery may mount DATA for diagnosis and repair according to policy and may expose individual user data only after the necessary authorization. The temporary recovery state disappears after reboot unless a deliberate persistent repair operation changed the protected storage.

## 108.18 User data protection and encryption extension

Users are isolated from each other by default. Administrative authority and access to private user data are separate capabilities.

Future encryption may exist at two scopes:

```text
whole-DATA encryption
per-user data encryption
```

Encryption is an extension point of the security architecture rather than a mandatory requirement of the current implementation phase.

## 108.19 Current development status

```text
Phase 1.1  — accepted and consolidated
Phase 1.2  — accepted and consolidated
Phase 1.3  — accepted and consolidated
Phase 1.4  — accepted and consolidated
Phase 1.5  — accepted and consolidated
```

Phase 1.5 establishes the accepted contracts between Security, Root Mapping, Application Runtime, System Runtime, UserSession, IPC, Events/Operations and Diagnostics/Health. Concrete Rust crate/API design begins in the next phase.


# 109. Phase 1.5 — Security, Root Mapping and Runtime Contracts

Phase 1.5 decisions are consolidated here as part of the current Source of Truth. The phase establishes the contracts between security policy, logical-root construction and runtime execution.

## 109.1 Security contract

`luna-security` uses a combined security architecture with separated internal concerns:

```text
luna-security
├── identity
├── authentication
├── authorization
├── capabilities
├── permissions
├── trust
├── credentials
├── grants
└── audit
```

It is implemented as a library + daemon/service component. Security is the policy authority; it does not own application lifecycle, namespace creation or filesystem mounting. Linux/kernel/filesystem primitives perform low-level enforcement.

The conceptual authorization model is:

```text
Subject + Resource + Action + Context
                 ↓
          PolicyDecision
```

`PolicyDecision` can represent allow, deny, user confirmation, or constrained access. User confirmation is UI-agnostic: GUI, CLI and recovery UI may present the same backend request in their own way.

Permissions are compositional across decision, duration and scope. Explicit higher-level DENY cannot be weakened by a lower-level ALLOW.

`Subject`, `Resource`, `Action`, `PolicyDecision`, `Grant`, `TrustRecord`, `Credential`, `Role` and `Capability` are foundational security concepts.

Administrator authority is represented by roles/capabilities rather than a permanent root user. Administrative elevation is per operation, not a reusable privileged session.

Protected user-data access and Recovery access are explicit authorization transactions. Recovery is a privileged repair environment but does not automatically unlock every user's private data.

Signatures, integrity, trust and authorization are separate concepts:

```text
Integrity
Authenticity
Trust
Authorization
```

Modified external bundles warn the user. The user may cancel, launch once, or explicitly create a local trust record. Local trust binds to the specific content identity/hash rather than silently trusting every future version.

## 109.2 Logical path and mapping contract

`luna-root-mapping` remains a narrow mapping component. It separates `LogicalPath` from `PhysicalPath` and uses typed mapping concepts rather than exposing raw host paths as application APIs.

Core conceptual objects are:

```text
LogicalPath
PhysicalPath
MappingRule
MappingPolicy
MappingPlan
MappingResult
MappingError
```

Mappings are file-oriented by default. Whole subtrees are allowed only when explicitly permitted by semantic mapping policy. Access vectors distinguish at least read, write and execute.

A `MappingRule` describes what logical resource should be supplied from what permitted source. A `MappingPlan` describes the runtime operations needed to realize those rules in a namespace. Policy is shared where it is inherently application/user-wide, while actual namespace state remains private to each `ApplicationInstance`.

The implementation direction is hybrid: Linux mount namespaces plus bind mounts, OverlayFS and other filesystem primitives may be used where suitable. These are implementation mechanisms, not the Luna architectural model. A mapping backend abstraction separates the mapping model from particular kernel mechanisms.

Mapping construction is deterministic. Explicit deny stops path resolution; a missing resource may fall back only through the semantic class's permitted source order. Duplicate identical mappings are deduplicated; conflicting mappings are errors. Upper-layer shadowing can override lower defaults without mutating the immutable source.

Path traversal and symlink escape outside an authorized physical boundary are forbidden. The mapping subsystem and filesystem backend must validate actual path resolution rather than only textual paths.

## 109.3 System configuration and mapping precedence

`DATA/system/config/` is the canonical location for mutable configuration that applies to the whole system, while:

```text
DATA/users/<user>/config/
```

contains user-specific configuration.

For applicable configuration mappings, the conceptual resolution order is:

```text
user DATA / override
        ↓
application-provided/default content
        ↓
DATA/system/config
        ↓
System Image default
```

The exact order is selected per semantic mapping class; `/home`, `/proc`, `/sys`, `/dev`, libraries and other system resources do not all use this same chain.

## 109.4 System Image materialization model

System Image remains a direct SquashFS image and is the immutable source of system content. It is not the final runtime root. Luna creates a logical Linux-compatible root in RAM/virtual filesystem state and materializes required system content into that runtime view.

The desired loading model is hybrid:

```text
System Image
     ↓
critical initial materialization
     ↓
RAM logical root
     ↓
manifest-informed prefetch
     ↓
demand-driven loading
```

Materialized system content can be reclaimed while the source System Image remains available. If the source Image is detached/deleted, all system content required for the continued running system must first be fully materialized and independently verified. After successful detach, those resident system files are no longer reclaimable solely on the assumption that the deleted Image can provide them again.

The precise kernel-level implementation of materialization is intentionally left for implementation. The architecture requires the semantics, not a premature commitment to one copy/cache mechanism.

## 109.5 Application bundle mapping

Application bundles describe their own logical interface rather than absolute DATA paths. For example:

```text
bundle source:
    resources/bin/app

logical executable:
    /usr/bin/app
```

The application sees the logical executable path and does not know that its physical payload is stored under `DATA/system/apps`, an external disk or a removable drive.

`luna-app-manager` is responsible for importing package inputs such as `.deb`/`.rpm`, assembling the bundle, generating its manifest and declared mapping requirements. `luna-root-mapping` later resolves those requirements for a particular runtime/user/security context and constructs the actual MappingPlan. The installed bundle remains immutable.

Shared immutable libraries may have one physical backing object consumed by multiple namespaces, while each namespace gets its own logical mapping. Different applications may map the same logical library path to different physical versions.

## 109.6 Application runtime contract

The canonical launch chain is:

```text
User / GUI / CLI
        ↓
luna-system-runtime
        ↓
UserSession
        ↓
luna-app-runtime
        ↓
ApplicationInstance
        ↓
Linux kernel
```

`luna-app-manager` is not part of normal execution. It manages installation, update, removal, verification, migrations and package import.

`ApplicationInstance` is the logical runtime entity that may contain multiple processes. The instance owns runtime-specific process/lifecycle state while policy, mappings and permission state may be shared when applicable to the application/user context.

Every `ApplicationInstance` gets its own filesystem/mount namespace. PID namespaces are not mandatory. Additional Linux namespaces are optional implementation capabilities and must not make the application behave as though it is inside a visible container or virtual machine. The application should instead perceive a normal Linux-compatible logical system root.

Resource control is established before `exec`. Application instances are attached to the user/application resource hierarchy using Linux resource-control mechanisms where practical.

## 109.7 UserSession model

The normal desktop runtime combines identity and session state into a single `UserSession` entity. The hierarchy is:

```text
luna-system-runtime
├── UserSession A
│   ├── app-runtime
│   │   ├── ApplicationInstance
│   │   └── ApplicationInstance
│   └── GUI/Desktop session
│
└── UserSession B
    ├── app-runtime
    │   ├── ApplicationInstance
    │   └── ApplicationInstance
    └── GUI/Desktop session
```

A normal PC startup leads directly into Luna's graphical welcome/login flow. The architecture does not depend on a user first reaching a TTY and then manually starting a Wayland compositor.

There is one system runtime coordinating multiple UserSessions. Separate independent system runtimes per user are not used. User-specific grants and session policy live in the UserSession context; there is no need for a separate parallel user-permission layer merely because identity and session were merged.

UserSession lifecycle still supports ACTIVE/continue, RESTRICTED and TERMINATED behaviour, with RESTRICTED as the default when a session is left. System services can continue independently of any one UserSession.

## 109.8 Runtime IPC and event model

Internal control-plane IPC uses Unix-domain sockets with a Luna-defined typed binary protocol. Protocol/API versions are explicit so independently updated components can negotiate supported contracts. Kernel-provided peer identity is combined with Luna identity/security policy for IPC authorization.

GUI and CLI are thin clients over the same backend services. D-Bus may be used for desktop compatibility, but it is not the primary internal control-plane contract.

The Luna event model uses a lightweight event-bus architecture. Kafka is a conceptual reference only; Kafka itself is not required inside the OS. Events have ordering metadata, audience/visibility and persistent or ephemeral classes where appropriate.

An `Operation` is a first-class asynchronous object for long-running work. Operations have identity, type, state, progress, owner and result/error information. Ownership may be system-level plus requesting UserSession; cancellation is phase-dependent.

## 109.9 Diagnostics and health

Diagnostics is a separate architectural subsystem/capability. It observes structured events, produces structured `DiagnosticReport` objects and proposes or coordinates bounded automatic repairs. It does not gain unlimited mutation authority merely because it is diagnosing a problem; actual changes are performed by the component that owns the relevant state, under Security authorization.

The system health model is:

```text
Healthy
Degraded
Recovering
Failed
Emergency
```

`Emergency` is a health/diagnostic state, not a Boot Menu mode. Boot selection remains separate. Health propagates according to scope: a single application or UserSession may be degraded without making unrelated sessions fail.

Diagnostics, logs and events are separate concepts. Security/audit and diagnostic data are subject to privacy-aware filtering. Users may export diagnostic reports, including to external storage, through the same permission model.

## 109.10 Phase 1.5 status

Phase 1.5 establishes the contracts and architectural invariants for:

```text
Security
Root Mapping
Application Runtime
System Runtime
UserSession
IPC
Events / Operations
Diagnostics / Health
```

The next phase is repository/crate architecture and the translation of these contracts into concrete Rust workspace boundaries and APIs.

# 110. Phase 1.6 — Repository, Crate Architecture and Implementation Boundary

Phase 1.6 is **ACCEPTED** through `1.6-HZ` based on the decisions accepted in the architecture discussion.

This section is the authoritative consolidation of Phase 1.6. The chronological answers remain traceability material; they are not a competing Source of Truth.

## 110.1 Phase 1.6 purpose

Phase 1.6 closes the architecture-first repository boundary and establishes the rules for moving from the accepted system contracts to a clean Rust workspace.

The order is mandatory:

```text
Phase 1.6 decisions
        ↓
ARCHITECTURE.md consolidation
        ↓
repository / Cargo audit
        ↓
luna-common audit and redesign
        ↓
new crate map from the current architecture
        ↓
crate/API contracts
        ↓
implementation
```

Old empty crates are not architectural commitments. A crate may be removed when it no longer represents the current responsibility model. Existing code may be retained only when it is useful to the current contract.

## 110.2 Workspace and repository principle

The Rust workspace must represent the **current** architecture, not historical component names or abandoned placeholders.

The repository is intentionally kept minimal while the architecture is being translated into implementation boundaries. Empty placeholder crates are not required merely to reserve names.

The workspace resolver is Rust-current (`resolver = "3"`). The existing repository may temporarily contain only the components that have useful current code. New components are introduced after their responsibility and API boundary are established.

## 110.3 `luna-common` boundary

`luna-common` remains a deliberately small foundational crate and must not become a dumping ground for unrelated system concepts.

Existing code in the old `luna-common` is treated as reusable source material, not as the final API. Existing identifiers such as IDs, versions and generic results/errors must be re-evaluated against the Phase 1.6 architecture before being retained.

`luna-common` may contain genuinely cross-cutting primitives shared by multiple crates. Subsystem-specific errors, policy objects, runtime state, filesystem operations, bundle semantics and service APIs belong to their owning crates.

A client-specific crate may be introduced separately when a client needs an API boundary distinct from the backend/library implementation.

## 110.4 Crate design principles

The crate map must follow architectural ownership rather than historical directory names.

A component that has both a reusable backend and a process/service boundary may use the accepted **small daemon/service + library** model. Thin CLI and GUI clients use the same backend rather than duplicating business logic.

Management and execution remain separate:

```text
manager  → state-changing management operations
runtime  → execution and lifecycle of running instances
security → policy authority
filesystem / kernel primitives → low-level enforcement
```

The crate boundary must not silently merge these responsibilities merely because they are convenient to implement together.

## 110.5 Accepted component direction

The architecture continues to use the following conceptual component names where their responsibilities require independent boundaries:

```text
luna-cli
luna-system-manager
luna-app-manager
luna-device-manager
luna-update-manager
luna-kernel-manager
luna-root-mapping
luna-security
luna-system-runtime
luna-app-runtime
luna-fs
luna-bundle
luna-config
luna-log
luna-common
```

This list is a **responsibility map**, not a command to create every crate immediately. The final Rust workspace is derived from the contracts and may split a component into library/service/client crates where the architecture requires it.

## 110.6 Runtime boundary retained

The accepted runtime hierarchy remains:

```text
luna-system-runtime
├── UserSession A
│   ├── app-runtime
│   └── GUI/Desktop session
└── UserSession B
    ├── app-runtime
    └── GUI/Desktop session
```

There is one system runtime supervising multiple UserSessions. Application execution belongs to `luna-app-runtime` and is represented by `ApplicationInstance` objects. `luna-app-manager` is not part of the normal launch chain.

Applications must receive a normal Linux-compatible logical environment rather than an intentionally visible container/VM identity. Linux namespaces may be used as implementation mechanisms without exposing a container model to the application.

## 110.7 Security boundary retained

`luna-security` remains the central policy authority. Security is a separate layer and must not be absorbed into runtime, mapping or filesystem crates merely for convenience.

Administrative authority does not require a permanent root user or a mandatory `sudo`/`su` model. Administrative credentials and per-operation authorization remain the architectural direction.

## 110.8 Mapping and filesystem boundary retained

`luna-root-mapping` remains a narrow logical-root and mapping component. `luna-fs` remains a low-level filesystem abstraction.

Linux mechanisms such as namespaces, mounts, bind mounts and related filesystem primitives are implementation mechanisms. They do not replace the Luna mapping model.

Mapping remains file-oriented, policy-controlled and namespace-specific. Shared information may be deduplicated where it is semantically global, while actual namespace state remains isolated.

## 110.9 Async and resource model

Asynchronous, multicore and multithreaded execution remains an explicit system goal. Tokio is accepted as the initial asynchronous runtime direction where an async runtime is required.

System resource protection remains a first-class architectural requirement. Linux mechanisms may be used initially to reserve CPU/memory/GPU capacity for system operation and responsiveness.

The system owns global resource reclamation rather than permanently assigning reclaimable runtime memory to whichever user is currently active.

## 110.10 Configuration and state

System-wide mutable configuration belongs under:

```text
DATA/system/config/
```

User-specific configuration belongs under:

```text
DATA/users/<user>/config/
```

Where a configuration value is resolved through layers, the accepted semantic precedence is:

```text
user override
    ↓
application/default content
    ↓
DATA/system/config
    ↓
System Image default
```

The exact precedence may be defined per semantic resource class; it is not a universal textual overlay rule.

Persistent state is preferred where it is the source of truth. State changes are event-driven rather than rewritten unnecessarily on every invocation.

## 110.11 Repository-to-architecture rule

Before implementing a crate, the repository must be audited against this document:

1. inspect root `Cargo.toml`;
2. inspect the actual workspace members;
3. inspect each surviving crate's source and manifest;
4. identify obsolete code and reusable code;
5. compare responsibilities against this Source of Truth;
6. remove or redesign stale boundaries before adding implementation;
7. only then define the new crate/API contract.

The repository must not be allowed to become a second, implicit architecture document.

## 110.12 Phase 1.6 accepted-answer ledger

The following ledger preserves the accepted Phase 1.6 answers so that the chronological answers cannot be lost even when phase working files are later archived. `ACCEPTED` means the user's answer accepted the proposal presented for that item. Where the user explicitly selected a variant or supplied a concrete implementation constraint, that selection is recorded verbatim in meaning.

### A–Z

```text
A  ACCEPTED
B  ACCEPTED
C  ACCEPTED
D  ACCEPTED
E  ACCEPTED
F  ACCEPTED
G  ACCEPTED
H  ACCEPTED
I  ACCEPTED
J  ACCEPTED
K  ACCEPTED — option B
L  ACCEPTED
M  ACCEPTED
N  ACCEPTED
O  ACCEPTED
P  ACCEPTED
Q  ACCEPTED
R  ACCEPTED
S  ACCEPTED
T  ACCEPTED
U  ACCEPTED
V  ACCEPTED
W  ACCEPTED
X  ACCEPTED
Y  ACCEPTED
Z  ACCEPTED
```

### AA–AZ

```text
AA ACCEPTED — option B
AB ACCEPTED
AC ACCEPTED
AD ACCEPTED
AE ACCEPTED
AF ACCEPTED
AG ACCEPTED
AH ACCEPTED
AI ACCEPTED
AJ ACCEPTED
AK ACCEPTED
AL ACCEPTED
AM ACCEPTED
AN ACCEPTED
AO ACCEPTED
AP ACCEPTED
AQ ACCEPTED
AR ACCEPTED
AS ACCEPTED
AT ACCEPTED
AU ACCEPTED
AV ACCEPTED
AW ACCEPTED
AX ACCEPTED
AY ACCEPTED
AZ ACCEPTED
```

### BA–BZ

```text
BA ACCEPTED
BB ACCEPTED
BC ACCEPTED
BD ACCEPTED
BE ACCEPTED
BF ACCEPTED
BG ACCEPTED
BH ACCEPTED
BI ACCEPTED
BJ ACCEPTED
BK ACCEPTED
BL ACCEPTED
BM ACCEPTED
BN ACCEPTED
BO ACCEPTED
BP ACCEPTED
BQ ACCEPTED
BR ACCEPTED
BS ACCEPTED
BT ACCEPTED
BU ACCEPTED
BV ACCEPTED
BW ACCEPTED
BX ACCEPTED
BY ACCEPTED
BZ ACCEPTED
```

### Ca–Cz

```text
Ca ACCEPTED
Cb ACCEPTED
Cc ACCEPTED
Cd ACCEPTED
Ce ACCEPTED — a separate client crate may be created when required
Cf ACCEPTED
Cg ACCEPTED
Ch ACCEPTED
Ci ACCEPTED
Cj ACCEPTED
Ck ACCEPTED
Cl ACCEPTED
Cm ACCEPTED
Cn ACCEPTED
Co ACCEPTED
Cp ACCEPTED
Cq ACCEPTED
Cr ACCEPTED
Cs ACCEPTED
Ct ACCEPTED
Cu ACCEPTED
Cv ACCEPTED
Cw ACCEPTED
Cx ACCEPTED
Cy ACCEPTED
Cz ACCEPTED
```

### Da–Dz

```text
Da ACCEPTED
Db ACCEPTED — Bin + lib
Dc ACCEPTED
Dd ACCEPTED
De ACCEPTED
Df ACCEPTED
Dg ACCEPTED
Dh ACCEPTED
Di ACCEPTED — Tokio accepted as the async runtime direction
Dj ACCEPTED
Dk ACCEPTED
Dl ACCEPTED
Dm ACCEPTED
Dn ACCEPTED
Do ACCEPTED
Dp ACCEPTED
Dq ACCEPTED
Dr ACCEPTED
Ds ACCEPTED
Dt ACCEPTED
Du ACCEPTED
Dv ACCEPTED
Dw ACCEPTED
Dx ACCEPTED
Dy ACCEPTED
Dz ACCEPTED
```

### Ea–Ez

```text
Ea ACCEPTED — option C
Eb ACCEPTED
Ec ACCEPTED
Ed ACCEPTED
Ee ACCEPTED
Ef ACCEPTED
Eg ACCEPTED
Eh ACCEPTED
Ei ACCEPTED
Ej ACCEPTED
Ek ACCEPTED
El ACCEPTED
Em ACCEPTED
En ACCEPTED
Eo ACCEPTED
Ep ACCEPTED
Eq ACCEPTED
Er ACCEPTED
Es ACCEPTED
Et ACCEPTED
Eu ACCEPTED
Ev ACCEPTED
Ew ACCEPTED
Ex ACCEPTED
Ey ACCEPTED
Ez ACCEPTED
```

### Fa–Fz

```text
Fa ACCEPTED
Fb ACCEPTED
Fc ACCEPTED
Fd ACCEPTED
Fe ACCEPTED
Ff ACCEPTED
Fg ACCEPTED
Fh ACCEPTED
Fi ACCEPTED
Fj ACCEPTED
Fk ACCEPTED
Fl ACCEPTED
Fm ACCEPTED
Fn ACCEPTED
Fo ACCEPTED
Fp ACCEPTED
Fq ACCEPTED
Fr ACCEPTED
Fs ACCEPTED
Ft ACCEPTED
Fu ACCEPTED
Fv ACCEPTED
Fw ACCEPTED
Fx ACCEPTED
Fy ACCEPTED
Fz ACCEPTED
```

### Ga–Gz

```text
Ga ACCEPTED
Gb ACCEPTED
Gc ACCEPTED
Gd ACCEPTED
Ge ACCEPTED
Gf ACCEPTED
Gg ACCEPTED
Gh ACCEPTED
Gi ACCEPTED
Gj ACCEPTED
Gk ACCEPTED
Gl ACCEPTED
Gm ACCEPTED
Gn ACCEPTED
Go ACCEPTED
Gp ACCEPTED
Gq ACCEPTED
Gr ACCEPTED
Gs ACCEPTED
Gt ACCEPTED
Gu ACCEPTED
Gv ACCEPTED
Gw ACCEPTED
Gx ACCEPTED
Gy ACCEPTED
Gz ACCEPTED
```

### Ha–Hz

```text
Ha ACCEPTED
Hb ACCEPTED
Hc ACCEPTED
Hd ACCEPTED
He ACCEPTED
Hf ACCEPTED
Hg ACCEPTED
Hh ACCEPTED
Hi ACCEPTED
Hj ACCEPTED
Hk ACCEPTED
Hl ACCEPTED
Hm ACCEPTED
Hn ACCEPTED
Ho ACCEPTED
Hp ACCEPTED
Hq ACCEPTED
Hr ACCEPTED
Hs ACCEPTED
Ht ACCEPTED
Hu ACCEPTED
Hv ACCEPTED
Hw ACCEPTED
Hx ACCEPTED
Hy ACCEPTED
Hz ACCEPTED
```

# Project Luna — Post-HZ Architecture Clarifications

**Date:** 2026-08-29
**Status:** ACCEPTED
**Authority:** These decisions are accepted architectural clarifications derived from the Project Luna Source of Truth and the 2026-08-29 repository/history audit. They must be consolidated into `docs/ARCHITECTURE.md` at the next architecture-document maintenance pass.

## 1. Runtime ownership

- `luna-system-runtime` is the single system-wide runtime/supervisor.
- There is no separate Session Manager.
- `UserSession` is the combined user/session domain entity.
- The runtime hierarchy is:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

- `luna-app-manager` is not part of normal application execution.
- `luna-app-runtime` owns normal application execution and ApplicationInstance lifecycle.

## 2. Logical root and application isolation

- Applications receive a conventional Linux-compatible logical `/` rather than an artificial reduced container filesystem.
- Luna's physical `DATA` layout remains Luna-native and is composed into the logical root through controlled mappings/materialization.
- The application must not be expected to know that its filesystem view is assembled by Luna.
- Linux namespaces, bind mounts and related kernel mechanisms are implementation primitives, not substitutes for Luna's mapping architecture.
- File mappings are the default.
- Explicit subtree/directory mappings are allowed for semantic resource classes such as shared library trees.
- Mapping tables are namespace-specific and RAM-resident at runtime.
- User/application/system precedence remains semantic-class-specific.
- User namespace usage must not be treated as a mechanism for granting ordinary applications root semantics.
- PID/user namespaces may be used for isolation, but exposing an artificial container identity to applications is not a Luna goal.
- `idmapped` mounts are allowed as an implementation primitive when they simplify secure ownership handling; they are not a mandatory Luna abstraction.

## 3. Canonical DATA state layout

The user-visible mutable DATA structure is:

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

- `DATA/system/config/` contains system-wide mutable configuration.
- `DATA/system/state/` contains persistent system state that is not ordinary configuration.
- `DATA/users/<user>/config/` contains user-specific configuration.
- `DATA/users/<user>/data/` contains user/application mutable data.
- `DATA/cache/` remains the common cache area, with semantic separation for system/user/application cleanup where required.

No alternate `DATA/data`, `DATA/apps`, `DATA/portable` or parallel duplicate tree is introduced.

## 4. Security and IPC/device visibility

- `luna-security` remains the central policy authority.
- Security decisions may be `Allow`, `Deny`, `Ask`, or structured constrained access according to the accepted policy model.
- An `Ask` decision is a request for explicit confirmation; Security itself remains UI-agnostic.
- D-Bus access should use a filtered/limited interface rather than expose the unrestricted host system bus to applications.
- `/dev` should be presented as a filtered device view exposing only resources authorized for the application/session.
- USB and external device access follows discovery → policy → authorized access; removable media does not implicitly execute arbitrary software.

## 5. Resource control

- Linux cgroups v2 and related kernel mechanisms are the initial enforcement primitives for CPU, memory and process/resource limits.
- A protected system-critical resource budget is reserved so applications cannot consume all resources and make the OS unresponsive.
- Memory reclamation remains globally controlled by the system rather than by the currently active user.
- Process-count limits and file-descriptor limits are accepted as ordinary resource safeguards.
- Disk/storage usage limits may be enforced where required through filesystem/resource facilities.

## 6. Persistent state implementation direction

- `luna-state` remains a synchronous storage abstraction with revision-checked atomic transactions.
- The first durable backend direction is a small embedded transactional key/value database.
- `redb` is the current implementation choice for this first backend/prototype.
- This backend choice is implementation-level and is not a new architectural boundary.
- A separate custom Luna WAL is not required when the selected backend already provides the durability guarantees required by the state contract.

## 7. Operations, boot success and recovery

- Boot success is not defined solely by kernel handoff. A userspace health/boot-success confirmation is required before a new boot target is considered confirmed.
- A watchdog/timeout can mark an unsuccessful boot attempt when the system fails to reach the required healthy state.
- Repeated application/runtime crashes eventually produce a user-visible recovery/diagnostic decision point according to policy; possible choices include restart, diagnostics, rollback and close.
- Recovery remains a dedicated Recovery System Image with temporary RAM-backed recovery state and a temporary recovery identity.
- Recovery is not Factory and is not a normal persistent user session.

## 8. Bundle and `.lbp`

- `luna-bundle` owns Bundle domain representation and format codec concerns.
- `luna-app-manager` owns install/update/remove/verify/migration/package-import lifecycle.
- No separate `.lbp` parser crate is introduced merely to parse the archive.
- `.lbp` is only the transport/archive representation of a Bundle.
- The installed Bundle is the immutable runtime unit.
- Bundle identity remains BundleId + Version + ContentIdentity.
- Bundle path/location does not define identity.

## 9. Reproducibility and provenance

- Reproducible-build metadata and artifact provenance are accepted as desirable implementation properties of the eventual signature/trust chain.
- Build metadata must not introduce nondeterministic content into ContentIdentity merely through timestamps or local filesystem paths.
- Publisher identity, repository/distribution metadata, content identity and local trust remain separate concepts.

## 10. Documentation and repository rules

- `docs/ARCHITECTURE.md` remains the single Source of Truth.
- Historical phase files preserve traceability and do not compete with the Source of Truth.
- `README.md`, `STATUS.md`, `ROADMAP.md`, `docs/architecture/CRATE-MAP.md` and implementation records must describe actual repository state.
- A stale document must be corrected rather than used as evidence for a new architectural decision.
- Repository implementation must not silently redefine accepted architectural responsibilities.

## 11. Current implementation sequence

```text
1. real Linux namespace + logical-root materialization
2. durable luna-state backend
3. real update/checkpoint/rollback engine
4. final Bundle Format v1 + RFC-0002 acceptance
5. production signature/trust chain
6. System Image/kernel compatibility + boot-state integration
7. final IPC/event transport
8. resource enforcement tuning
9. device/volume integration
10. end-to-end integration testing
```

# Project Luna — Phase 1.6 Crate Map

**Status:** architecture-driven implementation map
**Source of Truth:** `docs/ARCHITECTURE.md`

This document translates the accepted architecture into concrete Rust package boundaries. It is not a replacement for the architecture and must not introduce new architectural responsibilities.

## Foundation

| Crate | Responsibility | Form |
|---|---|---|
| `luna-common` | Small cross-cutting value types only | lib |
| `luna-fs` | Low-level filesystem abstraction and primitives | lib |
| `luna-root-mapping` | Logical filesystem/path mapping | lib |
| `luna-namespace` | Linux namespace/materialization mechanisms | lib |
| `luna-config` | Configuration model and scoped configuration | lib |

`luna-root-mapping` describes and resolves logical resources. It must not contain Linux namespace syscalls. `luna-namespace` contains the OS-specific enforcement/materialization primitives that consume validated mapping plans.

## Policy and management

| Crate | Responsibility | Form |
|---|---|---|
| `luna-security` | Central security/policy authority | lib/backend |
| `luna-app-manager` | Install, update, removal, verification, migrations and package import | lib + bin where required |
| `luna-system-manager` | System state model and queries | lib + bin where required |
| `luna-update-manager` | Executes system/application changes | lib + bin where required |
| `luna-kernel-manager` | Kernel inventory, metadata and compatibility queries | lib + bin where required |
| `luna-device-manager` | Device discovery, volumes and device lifecycle | lib + bin where required |

## Runtime

| Crate | Responsibility | Form |
|---|---|---|
| `luna-system-runtime` | Single system-wide supervision and `UserSession` orchestration | lib + bin where required |
| `luna-user-session` | `UserSession` domain model and lifecycle contract | lib |
| `luna-app-runtime` | `ApplicationInstance` execution/lifecycle boundary | lib + bin where required |

`luna-system-runtime` is the single system-wide runtime/supervisor. `UserSession` is the combined user/session entity.

Runtime ownership is intentionally separate from management ownership. `luna-app-manager` does not own normal application execution.

## Bundle

| Crate | Responsibility | Form |
|---|---|---|
| `luna-bundle` | Internal Bundle domain model, manifest/resource representation and eventual format codec | lib |

The crate exists in the current workspace. `.lbp` remains the transport/archive representation of a Bundle, and RFC-0002 has not yet been accepted as the final wire/archive specification.

## State and events

| Crate | Responsibility | Form |
|---|---|---|
| `luna-state` | Persistent state abstraction, revision and atomic transaction contracts | lib |
| `luna-event` | Event domain, subscriptions and delivery contracts | lib |

The current prototypes are in-memory/contract-level where the durable or OS-backed backend has not yet been implemented.

## User interface

| Crate | Responsibility | Form |
|---|---|---|
| `luna-cli` | Thin CLI client over backend APIs | lib + bin (`luna`) |

A future GUI client is separate and uses the same backend contracts.

## Boot

`luna-boot.efi` is a separate boot-specific project under `boot/luna-boot/`. It is intentionally outside the ordinary userspace workspace because it targets UEFI and operates before the userspace architecture exists.

The current boot implementation has progressed beyond the original scaffold: kernel loading and the test init handoff have been demonstrated through the shell (`sh`). Production trust/signature integration and final boot-compatibility work remain separate tasks.

`luna-boot-state` remains a conceptual architecture boundary and is not yet a separate workspace crate.

## Logging

`luna-log` is not created merely because the name existed historically. A dedicated logging boundary will be introduced when ownership/API requirements justify it.

## Dependency direction

```text
luna-common
    ↑
luna-fs
    ↑
luna-root-mapping
    ↑
luna-namespace

luna-config ───────┐
luna-security ─────┤
luna-state ────────┤
luna-event ────────┤
luna-bundle ───────┤
                   │
management crates ─┤
runtime crates ────┤
luna-cli ──────────┘
```

Higher-level crates consume lower-level contracts. No higher-level crate is allowed to pull application lifecycle, security policy, runtime state, bundle lifecycle or service APIs into `luna-common` or `luna-fs` merely for convenience.

## Current implementation rule

The repository may contain a scaffolded crate before its full backend implementation exists, but the scaffold must represent a responsibility boundary already defined by the architecture.

Before expanding a crate into a real backend, define:

1. responsibility;
2. public API;
3. state ownership;
4. persistence;
5. error model;
6. dependencies;
7. IPC/client boundary where applicable;
8. security boundary.

Existing implementation code is reusable source material, not an authority over the architecture.

## 110.13 Phase 1.6 status

```text
Phase 1.1 — accepted and consolidated
Phase 1.2 — accepted and consolidated
Phase 1.3 — accepted and consolidated
Phase 1.4 — accepted and consolidated
Phase 1.5 — accepted and consolidated
Phase 1.6 — accepted through HZ and consolidated
```

The project now moves from architectural decision closure to repository/crate audit. No implementation crate should be treated as final until it has been checked against this Source of Truth.

# END OF SOURCE OF TRUTH
