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
* системой, где System Image является `.lbp` bundle.

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
compatible Linux kernel
 ↓
Luna System Image
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

* выбрать System Image;
* выбрать совместимое ядро;
* перейти в recovery;
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

> System Images и kernels находятся в `system`, а не в `data`.

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
luna-3.0.0.squashfs
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

Это НЕ factory.

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
data/
├── system/
│   ├── apps/
│   └── drivers/
│
├── users/
│
├── data/
│
└── cache/
```

Это ВАЖНО.

System Images и kernels НЕ находятся здесь.

---

# 28. data/system

В:

```text
data/system/
```

находятся изменяемые компоненты пользовательской системы:

```text
data/system/
├── apps/
└── drivers/
```

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

# 48. Текущий Cargo workspace

На момент успешной сборки проект имел workspace примерно следующей структуры:

```text
project-luna/
├── Cargo.toml
├── components/
│   ├── luna/
│   ├── luna-bundle/
│   ├── luna-common/
│   ├── luna-config/
│   ├── luna-fs/
│   └── luna-log/
└── ...
```

В workspace были включены:

```text
components/luna
components/luna-bundle
components/luna-common
components/luna-config
components/luna-fs
components/luna-log
```

---

# 49. Уже существующие Rust components

На момент успешной сборки существовали:

```text
luna
luna-common
luna-log
luna-fs
luna-bundle
luna-config
```

Сборка успешно выполнялась:

```text
cargo build
```

и давала:

```text
Finished `dev` profile
```

---

# 50. Отключённые/ещё не созданные компоненты

В исходном workspace обсуждались компоненты:

```text
luna-boot
luna-init
luna-runtime
luna-session
lunad
```

Некоторые были закомментированы.

`luna-boot` остаётся архитектурно необходимым, но его реализация и отдельный workspace component ещё должны быть оформлены.

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

На текущем этапе фактически существуют только components, которые уже были созданы и успешно собирались:

```text
components/
├── luna/
├── luna-bundle/
├── luna-common/
├── luna-config/
├── luna-fs/
└── luna-log/
```

Следующие компоненты не нужно создавать заранее только потому, что они присутствуют в архитектурной схеме. Они появляются в workspace в момент начала их разработки.

Например, когда начнётся разработка bootloader, тогда появляется соответствующий компонент для `luna-boot.efi`. До этого в репозитории не должно быть пустой директории `luna-boot`.

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
* common error types;
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

# 58. Компонент: luna

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
* становиться `lunad`.

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

* точный бинарный формат `.lbp`;
* точная структура `.lbp`;
* точный TOML Bundle manifest;
* точный TOML System Image manifest;
* точный формат `current`;
* точный формат `factory`;
* точная структура kernel metadata;
* точная процедура определения kernel panic;
* точная процедура soft fallback;
* точный способ загрузки SquashFS в RAM;
* точная hybrid loading implementation;
* точный механизм application permissions;
* точный device automount backend;
* точная OpenRC integration;
* окончательный CLI `luna`;
* точная структура runtime namespaces;
* подписи bundles/images;
* cryptographic verification policy;
* update transaction protocol.

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
│   ├── system
│   │   ├── apps
│   │   └── drivers
│   │
│   ├── users
│   ├── data
│   └── cache
│
└── swap
```

В `system/` находятся только `images/` и `kernels/`.

---

# 72. Критически важные запреты для будущих чатов

Никогда автоматически не менять:

```text
system → data
```

для System Images.

Никогда не менять:

```text
System Image = SquashFS
```

на:

```text
System Image = .lbp
```

Никогда не делать:

```text
one global manifest
```

вместо:

```text
one manifest per System Image
```

Никогда не считать все kernels совместимыми со всеми System Images.

Никогда не заставлять bootloader переписывать state при каждом обычном boot.

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

Известные components:
- luna
- luna-common
- luna-log
- luna-fs
- luna-bundle
- luna-config

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

# 86. Что уже реально существует в коде

На текущем этапе важно не путать архитектурный план с уже реализованным кодом.

Реально был создан Rust workspace.

Уже успешно собирались:

```text
luna
luna-common
luna-log
luna-fs
luna-bundle
luna-config
```

Команда:

```bash
cargo build
```

успешно завершалась.

Это означает:

> базовый Rust workspace существует и собирается.

Это НЕ означает, что уже реализованы:

* bootloader;
* Bundle Format;
* System Image;
* kernel manager;
* runtime;
* device manager.

Они пока находятся на уровне архитектуры/планирования или ранней разработки.

---

# 87. История первых ошибок Cargo

Первоначально был ошибочный root `Cargo.toml`, потому что package не имел target.

Ошибка:

```text
no targets specified in the manifest
```

Причина:

отсутствовал:

```text
src/main.rs
```

или:

```text
src/lib.rs
```

или явный `[lib]`/`[[bin]]`.

После исправлений workspace начал собираться.

Также был файл:

```text
rust-toolchain.toml
```

который был пустым.

Это вызвало:

```text
empty toolchain override file detected
```

После устранения проблемы сборка прошла успешно.

Эти ошибки являются историей настройки проекта, а не архитектурными решениями.

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
│   └── volumes/
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

User sessions may coexist simultaneously. Each user can independently be configured for session behavior such as ACTIVE/continue, RESTRICTED, or TERMINATED. The default behavior is RESTRICTED.

System services are not tied to a single interactive user. An update transaction may continue while the active user changes.

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

Conceptual lookup precedence is:

```text
application
    ↓
user
    ↓
system
```

The model resembles layered lookup: the most specific layer satisfies the request first, with lower layers providing fallback.

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

Application execution is owned by `luna-app-runtime` together with the system runtime. The launch chain is therefore conceptually:

```text
user / file manager / CLI
        ↓
luna-app-manager (only when management is requested)
        ↓
luna-app-runtime
        ↓
luna-system-runtime
        ↓
ApplicationInstance
```

The exact IPC/API boundary remains to be specified.

`luna-app-runtime` is responsible for constructing the application execution environment, namespace, mappings, permissions and lifecycle state. `luna-system-runtime` supervises application runtimes and system-level runtime state. A separate supervisor component is not required initially; it may be introduced later if justified.

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
├── SYSTEM
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

# END OF SOURCE OF TRUTH
