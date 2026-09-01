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
graphical login UserSession
  ↓
authentication
  ↓
Active UserSession
  ↓
Wayland session
  ↓
niri
  ↓
Noctalia Shell
```

Штатная загрузка Luna не использует TTY как пользовательскую точку входа.
TTY/serial console может присутствовать только в development, diagnostic или recovery-сценариях.

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

---
# 110. Phase 1.6 — Архитектурная консолидация и принятые решения HZ

Phase 1.6 является текущей архитектурной базой после завершения большого цикла решений. Все решения Phase 1.6 до HZ считаются ПРИНЯТЫМИ. Они закрепляются именно в этом Source of Truth вместе с пояснениями и правилами разработки.

Хронологические phase-документы сохраняют ход обсуждения и нужны для traceability. Они не являются вторым текущим архитектурным источником.

### 110.1 Зачем была нужна Phase 1.6

Phase 1.6 переводит ранее принятые системные идеи в конкретные границы Rust-компонентов, доменных типов, менеджеров, runtime и API.

Порядок разработки сохраняется:

```text
Архитектура
    ↓
аудит репозитория
    ↓
границы crates
    ↓
доменные контракты
    ↓
manager/runtime contracts
    ↓
integration
    ↓
implementation
    ↓
hardening
```

Существующий код не считается архитектурным авторитетом сам по себе. Код должен соответствовать Source of Truth, а не наоборот.

### 110.2 Границы путей и mapping

Принято:

1. Linux-style logical paths проходят lexical normalization, после которой выполняется secure physical resolution.
2. `..` не может покинуть разрешённый physical root.
3. Symlink traversal за пределы разрешённой области требует boundary validation.
4. Mapping fallback является semantic-class-specific; универсальной цепочки поиска для всей файловой системы нет.
5. Application security identity основывается на `BundleId`; версия является дополнительным контекстом.
6. Разные версии приложения являются независимыми immutable runtime resources.
7. Logical paths являются Linux-style absolute paths.
8. Physical paths остаются внутренней реализационной деталью и не являются пользовательским API.
9. File mappings — значение по умолчанию.
10. Explicit directory/subtree mappings допускаются для подходящих semantic classes, например shared libraries.
11. Каждая application namespace имеет собственную RAM-resident mapping table.
12. Идентичные immutable mapping definitions могут безопасно переиспользоваться.
13. Конфликтующие mappings внутри одной namespace являются ошибкой.
14. System является нижним fallback precedence там, где fallback применим.
15. Application resources могут shadow-ить lower defaults, когда это разрешено semantic class.
16. User overrides могут shadow-ить lower defaults, когда это разрешено semantic class.
17. Общая последовательность `user → application → system` является semantic-class-specific, а не универсальным правилом overlay.
18. Security проверяется до окончательного принятия MappingPlan.
19. ApplicationInstance не изменяет принятую MappingTable на месте; изменение создаёт новую validated state/version.
20. `luna-root-mapping` не владеет application lifecycle.

### 110.3 Filesystem и domain types

Принято:

1. `luna-fs` владеет low-level filesystem paths и primitive operations.
2. `PhysicalPath` не выносится в `luna-common` только ради удобства.
3. `luna-fs` может работать с files, directories, metadata, symlinks и mode metadata, не принимая за `Security` решение о разрешении.
4. Filesystem errors остаются локальными `luna-fs`; единого глобального `LunaError` для всего проекта нет.
5. `luna-root-mapping` предоставляет типизированные logical/physical domain concepts.
6. `MappingTable` поддерживает insert/remove/lookup/conflict detection/validation.
7. Validated MappingTable неизменяем после runtime handoff; изменение означает новую table/version.
8. Immutable tables допускают atomic replacement.
9. Validated MappingPlan является immutable.
10. Security-policy changes могут потребовать повторную validation MappingPlan.
11. Bundle logical paths используют dedicated validated domain type.
12. Bundle-relative source paths используют dedicated domain type.
13. Bundle resources являются typed domain objects.
14. Raw host `PathBuf` не является универсальным BundleResource domain representation.

### 110.4 Namespace, materialization и execution

Принято:

1. Первая реализационная основа isolation — Linux mount namespaces + controlled bind mounts + Root Mapping.
2. OverlayFS может использоваться там, где он упрощает composition.
3. Собственный VFS откладывается за пределы текущей архитектурной версии.
4. `luna-namespace` владеет Linux-specific namespace/materialization primitives.
5. `luna-root-mapping` владеет mapping semantics и MappingPlan.
6. `luna-app-runtime` потребляет validated MappingPlan и не придумывает mappings.
7. Runtime environment должен выглядеть как обычный Linux-compatible logical root, а не как явно видимый контейнер.
8. Per-ApplicationInstance filesystem/mount namespace является обязательной основой application isolation.
9. Дополнительные namespaces являются policy-driven implementation mechanisms.
10. `ApplicationInstance` владеет своим execution/lifecycle state после создания.
11. Recovery не создаёт обычную persistent UserSession.
12. Recovery использует temporary identity.
13. Recovery State является RAM-only.
14. `luna-system-runtime` может перезапустить неисправный app-runtime без автоматического уничтожения UserSession.
15. Recovery runtime metadata не означает восстановление process memory.
16. Runtime restart предпочтительнее полного reboot, если восстановление действительно возможно.

### 110.5 Runtime hierarchy и UserSession

Принято:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

`luna-system-runtime` является единственным system-wide runtime/supervisor.

User и session являются одной доменной сущностью `UserSession`.

System-wide уникальность `ApplicationInstanceId` принадлежит system-runtime. Она не должна генерироваться независимо внутри per-user app-runtimes.

`UserSession` содержит user identity, session state и relevant policy/resource context.

Состояния сохраняются:

```text
ACTIVE
RESTRICTED
TERMINATED
```

По умолчанию уход из активной пользовательской сессии переводит её в `RESTRICTED`.

System services и update operations могут продолжать работу независимо от переключения пользователей.

### 110.6 State model

Принято:

1. `luna-state` представляет logical persistent state.
2. Checkpoint/rollback отделены от runtime state.
3. `luna-state` может содержать state domain + storage traits/backend implementations, сохраняя backend-agnostic domain semantics.
4. Storage для `luna-state` синхронное.
5. Поддерживаются minimal atomic transactions.
6. Revision-based optimistic concurrency является частью контракта.
7. EventId и OperationId независимы.
8. Persistent state не требует второго Luna-specific WAL поверх выбранного durable backend.
9. Текущий durable backend реализации — `redb`.
10. Persistent system state хранится под `DATA/system/state`.
11. В текущей реализации используется `DATA/system/state/luna-state.redb`.

### 110.7 Events и Operations

Принята полная модель:

```text
Event
  ↓
Bus
  ↓
history where appropriate
  ↓
subscribers / replay
```

Kafka используется только как концептуальная аналогия; Kafka не является обязательной системной технологией Luna.

Принято:

1. Event ordering является monotonic per operation там, где operation существует.
2. Для независимых операций нет обязательного глобального total order.
3. Timestamp — metadata, а не ordering authority.
4. Event classes: `Ephemeral`, `Persistent`, `Audit`.
5. Persistent history поддерживает replay/query.
6. Live subscriptions отделены от persistent history.
7. Subscriptions имеют explicit lifecycle.
8. Delivery использует bounded queues и backpressure.
9. Audit events нельзя silently drop.
10. Interrupted operations проходят reconciliation после runtime/service recovery.
11. Operations относятся к System или UserSession context, а не к GUI/CLI process lifetime.
12. Authorization различает `view`, `cancel`, `resume`, `rollback`.
13. Cooperative Cancel и Force Stop — различные действия.
14. Force Stop требует более сильной/emergency authorization и audit.
15. Operations при необходимости явно различаются как resumable, non-resumable или требующие reconciliation.
16. GUI/CLI disconnect не отменяет backend operation.

### 110.8 Manager boundaries и Plans

Принято:

```text
luna-app-manager
    ↓ ApplicationPlan

luna-update-manager
    ↓ UpdatePlan

luna-system-manager
    ↓ System State

luna-kernel-manager
    ↓ kernel inventory / compatibility

luna-device-manager
    ↓ device / volume lifecycle

luna-system-runtime
    ↓ runtime supervision / UserSession / instance identity
```

Правила:

1. `ApplicationPlan` принадлежит `luna-app-manager`.
2. `UpdatePlan` принадлежит `luna-update-manager`.
3. Plans не содержат low-level mount/syscall details.
4. App-manager строит и валидирует ApplicationPlan, но не запускает приложение.
5. ApplicationPlan проверяет dependencies, compatibility, security, resources и migrations до mutation.
6. Invalid plan не может перейти в mutation.
7. Update-manager исполняет mutation transaction, но не становится владельцем доменных semantics других managers.
8. Managers остаются владельцами state своих доменов.
9. High-level operation может включать несколько targets и per-target status, когда transactional semantics это допускают.

### 110.9 Update и rollback

Принята последовательность:

```text
prepare
   ↓
checkpoint
   ↓
apply
   ↓
verify
   ↓
commit
```

Принято:

1. Old authoritative state остаётся authoritative до commit, где это возможно.
2. После interruption выполняется reconciliation.
3. Reconciliation определяет committed / partially committed / not committed state.
4. Rollback является explicit operation, а не автоматической реакцией на каждый crash.
5. `luna-state` может ссылаться на checkpoint, но не владеет snapshot internals.
6. Btrfs является accepted implementation direction для persistent checkpoint/rollback.
7. System Image и kernel обновляются независимо.
8. Current/previous usable state не должен исчезнуть до подтверждённого commit.

### 110.10 Application identity и Bundle contracts

Принято:

1. ApplicationInstanceId отделён от Bundle identity.
2. UserSessionId не является manifest data.
3. ApplicationInstanceId не является manifest data.
4. Immutable Bundles reusable across UserSessions.
5. Разные версии приложения независимы и могут сосуществовать.
6. Application restrictions распространяются на все instances соответствующей application identity.
7. Instance-level policy может только усиливать restriction, но не ослаблять enforced deny.
8. Exact duplicate resource entries могут быть deduplicated; distinct targets остаются конфликтом.
9. Bundle не владеет physical installation paths.
10. Bundle parser и domain model не должны проникать в application lifecycle management.

### 110.11 Security model

`luna-security` является центральной policy authority.

Модель решения:

```text
Subject + Resource + Action + Context
                  ↓
           PolicyDecision
```

Принято:

1. Security policy revisioned.
2. Grants могут быть one-time, operation-scoped, while-running или persistent.
3. Trust связывает как минимум BundleId, ContentIdentity/hash и scope.
4. Trust является content-specific.
5. Integrity, signature validity, trust и authorization разделены.
6. `Ask` означает explicit confirmation; Security не знает, GUI это делает или CLI/recovery UI.
7. `Constrained` содержит structured typed restrictions.
8. Manifest mapping/capability declarations являются запросами, а не grants.
9. Security проверяется до final MappingPlan acceptance.
10. Policy changes могут требовать revalidation уже существующих mappings/plans.
11. Application-level restrictions применяются к соответствующим instances.
12. Instance не может ослабить policy.
13. Administrative authority основана на roles/capabilities, а не на permanent root user.
14. Постоянный архитектурный слой `sudo`/`su` не требуется.
15. Доступ Recovery к protected DATA также является отдельной authorization transaction.
16. User DATA может оставаться unencrypted по умолчанию; допускается whole-DATA и per-user encryption extension.
17. Administrator credential не должен быть пустым.
18. Возможность recovery административных credentials является частью принятого направления; точный cryptographic/authentication protocol остаётся открытым.

### 110.12 Resource protection

Принято:

```text
System
  ↓
User
  ↓
Application
  ↓
ApplicationInstance
```

Система резервирует protected resource budget для system-runtime, diagnostics и critical services.

Резерв адаптивный, а не универсальный фиксированный процент.

Для первоначального enforcement используются Linux resource-control mechanisms; `cgroups v2` — принятый baseline.

Memory pressure обрабатывается от disposable/reclaimable resources к application pressure и затем к controlled termination как последнему шагу.

Application instances не должны занимать защищённый system budget так, чтобы ломать управление системой.

### 110.13 Configuration

Принято:

```text
user override
    ↓
application/default
    ↓
DATA/system/config
    ↓
System Image default
```

Это не универсальный textual overlay: точная precedence является semantic-class-specific.

Machine-wide mutable configuration находится в `DATA/system/config`.

User-scoped configuration находится в `DATA/users/<user>/config`.

Изменение конфигурации не мутирует immutable System Image.

Удаление override возвращает соответствующий immutable default.

### 110.14 Device и external volume model

`luna-device-manager` владеет discovery, volume lifecycle и automount orchestration.

Принято пользовательское поведение:

```text
device connected
      ↓
detected
      ↓
filesystem detected
      ↓
automount
      ↓
friendly volume visible in file manager
```

Managed volume state находится в `DATA/system/volumes`.

Device Use является отдельным security dimension.

USB autorun не должен означать silent arbitrary execution.

Точный backend automount остаётся открытой технической задачей.

### 110.15 IPC и API versioning

Принято направление:

```text
Unix-domain sockets
        ↓
Luna-defined typed binary protocol
        ↓
explicit compatibility versions
```

Kernel peer identity комбинируется с Luna identity/security policy при IPC authorization.

GUI и CLI являются thin clients поверх backend contracts.

D-Bus допускается как desktop compatibility mechanism, но не является главным internal control-plane contract.

Публичные внутренние component contracts имеют explicit compatibility versions. Breaking changes требуют major compatibility change и явного отказа несовместимому клиенту.

### 110.16 Diagnostics и health

Diagnostics является отдельной capability/subsystem.

Она наблюдает structured events, создаёт structured `DiagnosticReport` и может координировать bounded repair, но не получает unlimited mutation authority только потому, что выполняет диагностику.

Health model:

```text
Healthy
Degraded
Recovering
Failed
Emergency
```

`Emergency` — health/diagnostic state, а не отдельный Boot Menu mode.

Failure одного application runtime не должен автоматически делать другие UserSessions неисправными.

### 110.17 Bundle Format v1 — RFC-0002

RFC-0002 — Bundle Format v1 — **ПРИНЯТ 2026-08-30**.

`.lbp` остаётся отдельным Bundle transport/archive format. System Image остаётся отдельным SquashFS artifact.

Приняты v1 инварианты:

```text
LBP1
64-byte little-endian header
64-byte section entries
TOML manifest
deterministic TAR payload
BLAKE3-256 integrity/content identity
zstd canonical compression
logical mapping declarations
request-only capabilities
optional Ed25519 signature
immutable installed Bundle
```

Принято:

1. `BundleId + Version + ContentIdentity` образуют identity Bundle.
2. ContentIdentity независим от filename и physical location.
3. Different Bundle versions may coexist.
4. Discovery не означает execution.
5. External Bundle flow: inspect → verify → trust → authorization → launch/install.
6. Bundle identity не зависит от физического носителя.
7. Parser обязан защищаться от overflow, out-of-bounds, overlapping sections, truncation, corrupt compression, traversal, absolute payload paths, duplicate paths, unsupported TAR entries, malformed TOML и invalid signature encoding.
8. Mappings — declarations, not permissions.
9. Requested capabilities — requests, not grants.
10. Signature validity, trust и permission policy остаются отдельными decision steps.

### 110.18 Bundle/runtime и security integration

`luna-app-manager` владеет installation/import/update/remove/verification/migration.

`luna-app-runtime` владеет normal execution lifecycle.

`luna-root-mapping` строит mapping plan.

`luna-namespace` материализует Linux-specific execution environment.

`luna-security` авторизует policy decisions.

Это даёт цепочку:

```text
Bundle declaration
      ↓
ApplicationPlan
      ↓
MappingPlan
      ↓
Security
      ↓
Namespace materialization
      ↓
ApplicationInstance
```

### 110.19 System Image loading

System Image по-прежнему является непосредственно SquashFS.

Hybrid loading означает:

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

Цель — не копировать весь System Image в RAM без необходимости.

Точный kernel-level materialization mechanism остаётся implementation/specification work.

Если активный Image отсоединяется или удаляется, required system content должен быть materialized и verified независимо до потери source. Уже resident content нельзя reclaim-ить только потому, что source Image больше недоступен.

### 110.20 Recovery

Recovery является отдельной working environment.

```text
Recovery System Image
        ↓
Recovery kernel / compatible boot path
        ↓
RAM logical root
        ↓
temporary recovery identity
        ↓
RAM-backed writable state
```

Принято:

1. Recovery может работать без normal persistent DATA.
2. User DATA не открывается автоматически.
3. Protected user DATA требует explicit authorization.
4. Recovery не равен Factory.
5. Recovery не является permanent root-user mode.
6. Factory сохраняет оригинальные known-good System Image и factory kernel.

### 110.21 Персонализация и application overrides

Установленный Bundle immutable.

Compatibility/metadata customization реализуется как controlled user override, а не изменение исходного immutable payload.

Если пользователь намеренно изменяет overrideable fields, это должно быть явно видно пользователю. Modified/overridden Bundle переходит в отдельное local/non-original trust state и требует explicit decision; local trust привязывается к конкретной ContentIdentity, а не молча ко всем будущим версиям.

### 110.22 Sources, channels и offline installation

System/application updates могут быть связаны с channels, например stable/beta/nightly/local, но точная taxonomy остаётся policy/configuration detail.

Источники могут быть official repository, third-party repository, local file, removable media и будущие network/share sources.

Verification/trust применяются до installation.

Offline installation поддерживается.

### 110.23 Что из прежних формулировок считать устаревшим

Исторический текст этого документа сохраняется, чтобы не терять объяснения и историю решений. Однако при конфликте с Phase 1.6 действуют следующие актуальные правила:

```text
System Image → direct SquashFS
Bundle       → .lbp
SYSTEM       → immutable/versioned images + kernels
DATA         → mutable state
UserSession  → combined user + session entity
system-runtime → единственный system-wide runtime/supervisor
app-runtime  → ApplicationInstance execution/lifecycle
security     → central policy authority
root-mapping → logical mapping
namespace    → Linux-specific materialization/enforcement
```

Старые placeholder-модели, которые уже были заменены этими решениями, остаются только как history и не должны использоваться для нового кода.

### 110.24 Полный реестр принятых решений Phase 1.6 — 1…115

```text
1   logical paths: lexical normalization + secure physical resolution
2   semantic-class-specific mapping fallback
3   BundleId-based application security identity
4   independent immutable application versions
5   Event → Bus → history where appropriate → subscribers/replay
6   luna-state = logical persistent state; checkpoint/rollback separate
7   app-manager builds/validates ApplicationPlan; update-manager executes changes
8   system-runtime creates/supervises UserSessions
9   system-runtime owns ApplicationInstanceId uniqueness
10  Linux resource-control mechanisms are the first resource-protection implementation
11  mount namespaces + controlled bind mounts + Root Mapping; OverlayFS where useful
12  System State, Boot State and Recovery State remain separate; Recovery State is RAM-only
13  logical paths are Linux-style absolute paths
14  physical paths remain internal implementation details
15  file mappings are default; directory/subtree mappings allowed where semantic
16  each application namespace has its own RAM-resident mapping table
17  identical immutable mapping definitions may be shared safely
18  the same logical dependency path may map to different physical versions per application
19  mapping conflicts inside one namespace are errors
20  System is lowest fallback precedence where fallback applies
21  application resources may shadow lower defaults where permitted
22  user overrides may shadow lower defaults where permitted
23  user → application → system precedence is semantic-class-specific
24  Security is checked before final MappingPlan acceptance
25  active ApplicationInstance cannot mutate its accepted MappingTable in place
26  app-runtime consumes validated MappingPlan and does not invent mappings
27  root-mapping does not own application lifecycle
28  ApplicationPlan belongs to app-manager
29  UpdatePlan belongs to update-manager
30  Plans do not contain low-level mount/syscall details
31  high-level operation may include multiple targets and per-target status where transactional semantics permit
32  managers remain owners of their domain state
33  system-runtime is ApplicationInstanceId authority
34  per-user app-runtimes do not generate global uniqueness
35  app-runtime owns ApplicationInstance lifecycle after creation
36  hierarchy = system-runtime → UserSession → app-runtime
37  UserSession contains user identity, state and resource/policy context
38  app-runtime does not create UserSessions
39  Recovery uses temporary identity, not normal persistent UserSession
40  Recovery uses RAM-backed state + explicit authorization before protected user data is opened
41  luna-fs owns low-level filesystem paths
42  PhysicalPath belongs to mapping/storage, not common
43  luna-fs supports files, directories, metadata, symlinks and mode metadata
44  filesystem errors are local to luna-fs; no global LunaError
45  root-mapping exposes typed logical/physical domain concepts
46  MappingTable supports insert/remove/lookup/conflict detection/validation
47  validated MappingTable is immutable after runtime handoff
48  immutable mapping tables permit atomic replacement
49  validated MappingPlan is immutable
50  security-policy changes can require MappingPlan revalidation
51  Security policy is revisioned
52  grants may be one-time/operation-scoped/while-running/persistent
53  Security owns policy/grants/trust, not runtime process state
54  trust binds BundleId + content identity/hash + scope
55  trust is content-specific
56  Bundle resources are typed domain objects
57  manifest separates identity/metadata/resources/dependencies/capabilities/entry points
58  ApplicationInstanceId is not manifest data
59  UserSessionId is not manifest data
60  immutable Bundles are reusable across UserSessions
61  luna-bundle does not own physical installation paths
62  app-manager constructs/validates ApplicationPlan without launching apps
63  ApplicationPlan validates dependencies/compatibility/security/resources/migrations
64  invalid plans cannot enter mutation
65  UpdatePlan belongs to update-manager
66  update stages = prepare/checkpoint/apply/verify/commit
67  old authoritative state remains authoritative before commit where possible
68  reconciliation determines committed/partially committed/not committed after interruption
69  rollback is explicit, not automatic for every crash
70  luna-state may reference checkpoints but does not own snapshot internals
71  events carry correlation metadata
72  live subscriptions are separate from persistent event history
73  subscriptions have explicit lifecycle
74  GUI/CLI disconnect does not cancel backend operations
75  Option C: luna-state contains state domain + storage traits/backend implementations
76  luna-state uses synchronous storage
77  state storage supports minimal atomic transactions
78  revision-based optimistic concurrency is supported
79  EventId and OperationId are independent
80  event ordering is monotonic per operation; no global total order across independent operations
81  timestamp is metadata, not ordering
82  event classes = Ephemeral / Persistent / Audit
83  persistent event history supports replay/query
84  event delivery uses bounded queues/backpressure
85  Audit events cannot be silently dropped
86  interrupted operations are reconciled after service/runtime recovery
87  operations distinguish resumable/non-resumable/unknown where necessary
88  operations belong to System or UserSession context, not GUI/CLI process lifetime
89  operation authorization distinguishes view/cancel/resume/rollback
90  Force Stop is distinct from cooperative Cancel and needs stronger/emergency authorization
91  Bundle logical paths use dedicated validated domain type
92  Bundle-relative source paths use dedicated domain type
93  Bundle resources carry explicit resource types/metadata
94  conflicting same-logical-path mappings within one namespace are errors
95  exact duplicate resource entries may be deduplicated; distinct targets remain conflicts
96  Bundle identity = BundleId + Version + ContentIdentity
97  ContentIdentity is independent of filename and physical storage location
98  moving a Bundle does not change its identity
99  external Bundle flow = inspect → verify → trust decision → launch
100 permissions distinguish Visibility / Read / Write / Execute / DeviceUse / Manage
101 application-level restrictions propagate to all instances of that application identity
102 an instance may tighten but may not weaken application policy
103 security policy revision participates in revalidation
104 Ask = explicit confirmation; Security remains UI-agnostic
105 Constrained = structured typed restrictions
106 ApplicationInstance lifecycle = Created / Starting / Running / Stopping / Stopped / Crashed / Failed
107 system-runtime may restart failed app-runtime
108 runtime metadata recovery does not restore application process memory
109 system-runtime restart is preferred over unnecessary full-machine reboot
110 protected system-critical resource budget is reserved
111 resource reservation is adaptive, not a universal fixed percentage
112 memory-pressure reclamation proceeds from disposable/reclaimable resources toward application pressure and controlled termination
113 GUI and CLI use shared backend contracts and do not directly operate on filesystem/runtime internals
114 luna supports machine-readable output in addition to human-readable output
115 public internal component contracts have explicit compatibility versions; breaking changes require explicit major compatibility change
```

### 110.25 Выбранные решения вариантов Phase 1.6

Особенно важные явные выборы:

```text
1.6-K   = B
1.6-AA  = B
1.6-Db  = Bin + lib
1.6-Di  = Tokio
1.6-Ea  = C
```

Эти выборы являются частью принятой Phase 1.6 baseline.

### 110.26 Явно отвергнутые технические предложения

Чтобы не повторять ошибки при дальнейшем развитии:

* `SystemState.previous` не является полной fallback-моделью; fallback использует inventory и compatibility queries и не ограничен одним previous image.
* Raw host `PathBuf` не является универсальным BundleResource representation.
* Обязательный `into_string()` для каждого wrapper не является архитектурным правилом.
* Обязательный `const fn` везде, где он технически возможен, не является стилевым/архитектурным правилом.

---
# 111. Post-HZ уточнения, принятые в последующих обсуждениях

После закрытия основной Phase 1.6 появились дополнительные уточнения, которые теперь считаются частью текущего SoT.

### 111.1 Durable state

`luna-state` использует durable `redb` backend. Persistent system state находится под `DATA/system/state/`, а текущая реализация использует `luna-state.redb`.

Это не меняет domain semantics `luna-state`: state model остаётся отделённой от checkpoint/snapshot internals.

### 111.2 Update journal

`luna-update-manager` должен писать durable intent до destructive/state-changing mutation и сохранять прогресс операции достаточно подробно для interruption reconciliation.

Текущая реализация использует durable operation state для intent и applied/inflight progress. Точный backend-domain wiring ещё продолжается.

### 111.3 Namespace/runtime integration

`luna-app-runtime` имеет security-aware namespace preparation boundary.

Не-`Allow` security decisions должны fail closed до namespace materialization.

Process creation/supervision остаются runtime responsibility; `luna-namespace` не превращается в process manager.

### 111.4 Bundle implementation

`luna-bundle` уже содержит LBP1 reader/writer baseline для принятого RFC-0002.

Оставшаяся работа — conformance/security hardening, signature verification/trust binding и application-manager integration.

### 111.5 Bootloader

`luna-boot.efi` развивается отдельной веткой работы и уже достигает Linux kernel + test init + `sh`.

Это implementation status, а не изменение фундаментальной архитектуры.

### 111.6 Rust workspace

Текущий userspace workspace архитектурно определён следующими crates:

```text
luna-common
luna-fs
luna-root-mapping
luna-namespace
luna-config
luna-security
luna-state
luna-event
luna-bundle
luna-app-manager
luna-system-manager
luna-update-manager
luna-device-manager
luna-kernel-manager
luna-system-runtime
luna-user-session
luna-app-runtime
luna-cli
```

`luna-boot.efi` находится вне обычного userspace workspace.

Отдельный `luna-log` crate не является обязательной текущей архитектурной границей только из-за исторического имени.

### 111.7 Текущие архитектурные invariants

```text
Project Luna
    ↓
small stable immutable foundation
    ↓
EFI / SYSTEM / DATA / SWAP
    ↓
custom luna-boot.efi
    ↓
versioned System Images = direct SquashFS
    ↓
per-image manifests
    ↓
independent versioned kernels
    ↓
current + factory
    ↓
compatibility-aware fallback
    ↓
.lbp Bundle Format v1
    ↓
central Security policy
    ↓
logical Root Mapping
    ↓
Linux namespace materialization
    ↓
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

### 111.8 Текущие открытые вопросы

После Phase 1.6 открытыми остаются только детали, которые действительно не были закрыты:

* точная финальная схема System Image manifest;
* точная финальная схема kernel metadata;
* точный persistent boot-state format;
* exact boot-success confirmation mechanics;
* exact technical soft-fallback implementation;
* exact hybrid materialization mechanism;
* exact automount backend;
* окончательная CLI syntax;
* точная OpenRC-like service integration;
* точный recovery-key/authentication protocol;
* production signature/trust integration details;
* точные domain UpdateBackend implementations;
* окончательная transport-level реализация IPC/event слоя.

Нельзя выдавать эти пункты за уже принятые архитектурные решения.

---
# 112. Rust Learning Rules — обязательная часть разработки Project Luna

Rust является не только языком реализации Project Luna, но и языком обучения пользователя в процессе разработки. Поэтому существенный Rust-код должен быть понятным и объяснимым.

### 112.1 Главное правило

> При существенном изменении Rust-кода нужно объяснять не только что сделано, но и почему выбран именно такой вариант.

### 112.2 Что объяснять по месту

При существенных изменениях объясняются:

* `struct` и причины выбранной структуры данных;
* `enum` и модель состояний;
* `Option<T>`;
* `Result<T, E>`;
* ownership;
* borrowing (`&T`, `&mut T` и передача ownership);
* lifetimes, если они действительно влияют на дизайн;
* traits и границы абстракций;
* modules и crates;
* `Arc`, `Mutex`, channels и другие concurrency tools;
* `async/await` и Tokio там, где asynchronous execution реально нужен.

### 112.3 Сравнение с Python

У пользователя есть Python background, поэтому сравнение с Python можно использовать для объяснения модели Rust:

```text
Python
object + dynamic references

Rust
value + explicit ownership + static types
```

Сравнение должно помогать понять Rust, а не превращаться в механический перевод каждой строки.

### 112.4 Стиль Rust-кода

Предпочтение отдаётся коду, который:

* явно показывает ответственность типов;
* использует понятные имена;
* избегает ненужных macro/abstraction layers;
* не скрывает существенную логику за "магией";
* имеет небольшие функции с ясной задачей;
* явно обрабатывает ошибки;
* закрепляет важные контракты тестами.

Сложные конструкции Rust допустимы, когда их необходимость можно объяснить.

### 112.5 Формат объяснения существенного Rust patch

```text
1. Что делает изменение.
2. Какие типы появились/изменились.
3. Кто владеет данными.
4. Где используется borrowing.
5. Что возвращает Result/Option и почему.
6. Где проходит crate/module boundary.
7. Какие тесты проверяют контракт.
```

### 112.6 Rust и архитектурные границы

```text
architecture boundary
        ↓
crate boundary
        ↓
module boundary
        ↓
typed API
        ↓
implementation
```

Если код начинает обходить эти границы через global state, скрытые side effects, неясные host paths или общий dumping-ground type, это сигнал к архитектурному пересмотру.

---
# 113. Текущее состояние проекта после Phase 1.6-HZ

Проект находится уже не на стадии чистого проектирования: архитектурный цикл принят, фундаментальные contracts существуют, а реализация продолжается.

### Уже существует в значимой степени

```text
luna-namespace
    Linux namespace/materialization primitives

luna-security
    policy/permission/trust baseline

luna-app-runtime
    security-aware namespace preparation boundary

luna-state
    durable redb state backend

luna-update-manager
    durable operation intent/progress + update/checkpoint orchestration

luna-bundle
    LBP1 codec baseline

luna-boot.efi
    kernel load + test init + sh prototype
```

### Следующая рабочая последовательность

```text
1. security-authorized child-process creation + supervision
2. durable state integration with runtime and domain managers
3. domain-backed UpdateBackends
4. LBP1 conformance/security hardening + Ed25519 trust binding
5. System Image/kernel manifests + boot-success/boot-state mechanics
6. IPC/event transport
7. cgroups/resource enforcement
8. filtered device population + volume integration
9. end-to-end Linux/QEMU validation
```

---
# 114. Правило поддержания этого Source of Truth

Этот файл остаётся **подробным архитектурным документом**, а не кратким README.

Он должен сохранять:

```text
архитектуру
+
объяснения
+
правила разработки
+
принятые решения
+
необходимую историю уточнений
```

При закрытии новой архитектурной фазы:

```text
phase decisions
      ↓
проверка конфликтов
      ↓
обновление этого файла
      ↓
phase document → history / traceability
```

Не нужно вычищать отсюда полезные объяснения только ради краткости. Удаляется или переписывается только информация, которая действительно стала ложной, устаревшей или заменена последующим явным решением.

При изменении принятого решения новая версия должна явно фиксировать:

```text
старое решение
→ superseded
→ новое решение
→ причина
→ затронутые компоненты
```

# END OF SOURCE OF TRUTH

# CURRENT DESKTOP SESSION MODEL

Этот раздел фиксирует текущую реализационную модель графической системы Luna.

```text
luna-system-runtime
        ↓
graphical login UserSession
        ↓
authentication
        ↓
Active UserSession
        ↓
Wayland session
        ↓
niri
        ↓
Noctalia Shell
```

Экран входа является частью жизненного цикла `UserSession`. После успешной
authentication эта же `UserSession` переходит в `Active`, после чего запускается
графическая Wayland-сессия.

TTY не является частью штатного пользовательского интерфейса или штатного способа
запуска рабочего стола. Serial/TTY доступ допускается только для разработки,
диагностики и Recovery.

`niri` является compositor/window manager графической сессии, а `Noctalia Shell`
является пользовательским desktop shell/UI. Они интегрируются в существующую
модель UserSession и system-runtime и не получают ответственность за системное
состояние, security policy или application installation.

