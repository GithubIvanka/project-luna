# Project Luna — архитектурный Source of Truth

**Проект:** Project Luna  
**Внутреннее имя:** `luna`  
**Язык разработки:** Rust  
**Лицензия:** Apache License 2.0  
**Статус:** текущий архитектурный Source of Truth  
**Редакция:** консолидация архитектуры и подготовки Phase 0, 2026-09-03

> **Правило:** сначала проектирование, затем код.

Этот документ является главным источником истины для архитектуры Project Luna. Принятое решение нельзя молча заменять реализацией. Если код показывает реальный архитектурный конфликт, конфликт сначала оформляется как отдельный вопрос и решается явно.

Исторические записи, старые этапы, ADR и RFC сохраняются для трассируемости. Они не должны противоречить текущей принятой архитектуре; если исторический текст отличается от текущего Source of Truth, для реализации используется текущий Source of Truth и актуальные принятые решения.

---

# 1. Назначение Luna

Project Luna — современная неизменяемая операционная система поверх Linux kernel. Luna не является обычным Linux-дистрибутивом с другим набором пакетов: её архитектура определяет собственную модель System Image, загрузки, состояния системы, bundles, изоляции приложений, обновлений и пользовательской файловой среды.

Основные цели:

1. очень маленькая и стабильная системная основа;
2. максимальная неизменяемость SYSTEM;
3. версионированные System Images;
4. независимые обновления System Image и kernel;
5. приложения в виде Luna Bundles;
6. изолированные зависимости;
7. контролируемая пользовательская файловая система;
8. собственный UEFI-загрузчик;
9. автоматическое управление внешними устройствами;
10. изоляция приложений через Linux primitives;
11. единый пользовательский инструмент `luna`;
12. графический и тихий обычный boot;
13. восстановление после неудачных обновлений;
14. независимость пользовательских данных от версии System Image.

Luna не должна превращаться в Docker-подобную систему, в классическую mutable Linux root filesystem или в проект, который переписывает всё поведение Linux с нуля.

---

# 2. Физическая модель диска

Установка Luna имеет четыре основные области:

```text
Disk
├── EFI
├── SYSTEM
├── DATA
└── SWAP
```

`SWAP` необязателен и может быть реализован как раздел, файл или ZRAM. EFI и SYSTEM являются OS-managed областями. DATA — основная изменяемая и пользовательски значимая область.

Допускается размещать EFI/SYSTEM и DATA/SWAP на разных физических дисках. Это не меняет логическую модель Luna.

---

# 3. EFI

EFI предназначен для UEFI boot infrastructure.

Канонически:

```text
EFI/
└── Luna/
    └── luna-boot.efi
```

EFI не является обычным пользовательским хранилищем. В штатной работе пользователь не должен редактировать EFI вручную.

---

# 4. SYSTEM

SYSTEM содержит только OS-managed, versioned и boot-critical данные.

Каноническая логическая структура:

```text
SYSTEM/
├── images/
│   ├── luna-X.Y.Z.squashfs
│   ├── luna-X.Y.Z.toml
│   └── ...
└── kernels/
    └── ...
```

Главные инварианты:

- System Image находится непосредственно в SYSTEM;
- payload System Image — непосредственно SquashFS;
- рядом с каждым image находится свой TOML manifest;
- kernels находятся в `SYSTEM/kernels/`;
- SYSTEM не является обычной writable пользовательской root filesystem;
- SYSTEM и DATA имеют независимые жизненные циклы.

---

# 5. System Image

System Image — это неизменяемый filesystem payload одной версии Luna.

Канонический файл:

```text
luna-X.Y.Z.squashfs
```

Он является непосредственно SquashFS filesystem image.

Запрещено считать System Image:

- `.lbp` Bundle;
- Bundle, внутри которого лежит SquashFS;
- произвольный контейнер, в котором SquashFS является внутренним payload.

Корректная модель:

```text
SYSTEM/images/
├── luna-1.0.0.squashfs
├── luna-1.0.0.toml
├── luna-2.0.0.squashfs
└── luna-2.0.0.toml
```

SquashFS содержит неизменяемую системную userspace среду, необходимую для построения logical `/`. Пользовательские документы, изменяемое состояние, application data и cache в образ не входят.

---

# 6. Manifest System Image

Каждый System Image имеет отдельный manifest непосредственно рядом с payload:

```text
luna-X.Y.Z.squashfs
luna-X.Y.Z.toml
```

Manifest нужен в том числе для работы `luna-boot.efi` до монтирования самого image.

Семантически manifest должен описывать как минимум:

- имя семейства системы;
- версию;
- архитектуру;
- формат payload;
- совместимые kernels;
- boot-related metadata;
- целостность/доверие, если это определено соответствующей политикой.

**Важно:** точная TOML-схема пока не считается принятым контрактом. Нельзя придумывать обязательные поля только потому, что они удобны одной реализации.

Черновой контракт вынесен в `docs/contracts/SYSTEM-IMAGE-CONTRACT.md`.

---

# 7. Linux kernels

Kernel является отдельным versioned артефактом и не является частью System Image.

Каноническая граница:

```text
SYSTEM/images/   → System Images
SYSTEM/kernels/  → Linux kernels
```

Для x86_64 PC текущий boot path использует стандартный Linux `arch/x86/boot/bzImage`, а `luna-boot.efi` реализует Linux x86_64 boot protocol.

Kernel metadata и точная схема каталогов ещё подлежат отдельной фиксации.

Черновой контракт: `docs/contracts/KERNEL-CONTRACT.md`.

---

# 8. Независимость image и kernel

System Image и kernel обновляются независимо.

```text
System Image A ── compatible ── Kernel 1
System Image A ── compatible ── Kernel 2
System Image B ── compatible ── Kernel 2
```

Сам факт наличия двух файлов не означает совместимость.

Bootloader обязан выбирать только явно допустимые пары.

---

# 9. `current` и `factory`

`current` — текущая подтверждённая рабочая комбинация System Image + kernel.

`factory` — сохранённая заводская, известная рабочая комбинация System Image + kernel.

Логически:

```text
current:
    image = luna-3.0.0
    kernel = 8.2.0

factory:
    image = luna-1.0.0
    kernel = 7.0.0
```

`factory` не удаляется обычной retention policy.

Точный физический формат записи этих ссылок определяется Boot State Contract.

---

# 10. Boot State

Boot State хранит только информацию, которая действительно нужна для выбора следующего boot target и обработки загрузочных отказов.

Обычная успешная загрузка не должна переписывать boot state.

Состояние меняется по существенным событиям:

- подготовка активации;
- смена `current`;
- подтверждение новой версии;
- подтверждённый boot failure;
- rollback;
- переход в Factory/Recovery.

Boot State не является заменой `luna-state` и не должен превращать bootloader в универсальную database engine.

Черновой контракт: `docs/contracts/BOOT-STATE-CONTRACT.md`.

---

# 11. luna-boot.efi

`luna-boot.efi` — самостоятельный UEFI-компонент, находящийся вне обычного userspace workspace.

Он отвечает за:

1. UEFI boot boundary;
2. обнаружение SYSTEM;
3. обнаружение System Images и kernels;
4. чтение manifests;
5. выбор совместимой image/kernel пары;
6. обработку клавиши `B`;
7. Boot Menu;
8. boot-time fallback;
9. формирование Linux boot context;
10. загрузку Linux kernel;
11. переход через `ExitBootServices`;
12. передачу управления kernel.

Он не отвечает за:

- UserSession;
- application lifecycle;
- Bundle installation;
- desktop;
- обычный system service management;
- GUI session management.

---

# 12. Обычная загрузка

Обычный boot должен быть быстрым, тихим и без задержки из-за Boot Menu.

```text
Power
  ↓
UEFI
  ↓
luna-boot.efi
  ↓
совместимый kernel + System Image
  ↓
luna-init
  ↓
logical /
  ↓
luna-system-runtime
  ↓
graphical login
  ↓
UserSession
  ↓
Wayland
  ↓
niri
  ↓
Noctalia Shell
```

При входе `luna-boot.efi` делает неблокирующее чтение доступного UEFI input buffer. Если `B`/`b` уже нажата, открывается Boot Menu. Иначе обычная загрузка продолжается без искусственной задержки.

---

# 13. Boot Menu

Boot Menu — исключительный путь, а не штатный пользовательский интерфейс.

Принятый порядок действий:

```text
1. Продолжить загрузку Luna
2. Подробная загрузка
3. Выбор System Image
4. Recovery Environment
5. Factory Environment
6. Загрузка с USB / внешнего устройства
```

При выборе System Image пользователю показываются только совместимые kernels.

Подробная загрузка подавляет обычный графический splash и включает расширенную диагностику только для этой попытки загрузки.

Recovery и Factory — отдельные режимы загрузки. Они не должны эмулироваться обычным TTY fallback.

---

# 14. Правило `ExitBootServices`

После `ExitBootServices` `luna-boot.efi` не должен обращаться к UEFI Boot Services, Boot Services allocator, console APIs или UEFI filesystem protocols.

Все данные, необходимые kernel handoff, должны быть подготовлены заранее и находиться в памяти, доступной после выхода из Boot Services.

Черновой контракт: `docs/contracts/BOOT-HANDOFF-CONTRACT.md`.

---

# 15. Передача System Image в userspace

`luna-boot.efi` не обязан монтировать SquashFS.

Логика:

```text
luna-boot
   ↓
выбранный kernel
   ↓
boot parameters + image context
   ↓
luna-init
   ↓
SYSTEM
   ↓
выбранный .squashfs
   ↓
logical /
```

Это оставляет filesystem/root construction в Linux userspace и не превращает UEFI-загрузчик во вторую ОС.

---

# 16. Failure и fallback

Неудачи загрузки разделяются по классу.

### Ошибка System Image

Если подходящий kernel уже запущен и failure относится к System Image, Luna должна, где это технически и безопасно возможно, попробовать предыдущий совместимый System Image без полного перезапуска.

```text
current image
    ↓ failure
previous compatible image
    ↓ failure
следующий compatible fallback
```

### Ошибка kernel

Kernel panic и другие kernel-level failures могут потребовать reboot. После перезапуска `luna-boot.efi` использует Boot State и выбирает предыдущую совместимую комбинацию.

### Исчерпание fallback

После исчерпания usable вариантов применяется Factory. Если Factory также недоступна, выбирается Recovery.

Черновой контракт: `docs/contracts/FAILURE-RECOVERY-CONTRACT.md`.

---

# 17. Early userspace — `luna-init`

`luna-init` — минимальный standalone musl binary раннего userspace.

Он не является обычным system supervisor.

Основные обязанности:

- ранняя подготовка окружения;
- поиск/подключение SYSTEM;
- открытие выбранного SquashFS;
- построение logical root;
- подключение DATA;
- подготовка минимальных kernel filesystems;
- `switch_root`;
- запуск `luna-system-runtime`.

`luna-init` намеренно остаётся отдельным компонентом и не обязан становиться обычным workspace crate.

---

# 18. Logical root

Физическое дерево разделов не является Linux root приложения или пользователя.

Luna строит logical `/`:

```text
physical SYSTEM + DATA
        ↓
logical-root mapping
        ↓
logical Linux /
```

Приложение затем получает ещё более ограниченное представление через собственный mount namespace.

Архитектура допускает hybrid/lazy доступ к SquashFS вместо обязательной полной копии image в RAM. Это направление должно оставаться совместимым с ограничениями памяти и lifetime materialization.

---

# 19. DATA

DATA содержит всё изменяемое состояние, которое должно переживать смену System Image.

Каноническая логическая структура:

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

### `DATA/system/apps/`

Общие установленные immutable Bundles. Один установленный Bundle не копируется отдельно каждому пользователю.

### `DATA/system/drivers/`

OS-managed mutable driver entities. Точная граница между kernel modules, firmware и userspace driver content ещё подлежит отдельному контракту.

### `DATA/system/libs/`

Адресуемые shared dependencies, которые не должны превращаться в одну конфликтующую глобальную библиотечную кучу.

### `DATA/system/volumes/`

Внутреннее состояние подключённых внешних томов. Пользовательский вид предоставляется volume/file-manager layer.

### `DATA/system/config/`

Изменяемая машинная конфигурация.

### `DATA/system/state/`

Долговечное системное состояние через `luna-state`.

### `DATA/users/<user>/`

Пользовательские данные и настройки. Они не зависят от версии System Image.

### `DATA/cache/`

Удаляемый cache. Cache не является durable state.

---

# 20. `luna-system-runtime`

`luna-system-runtime` — единственный system-wide supervisor Luna.

Архитектурная иерархия:

```text
luna-system-runtime
└── UserSession
    └── luna-app-runtime
        └── ApplicationInstance
```

Не существует отдельного generic `luna-runtime`, `luna-session` или другого универсального runtime daemon.

`RuntimeKind` и `RuntimeSpec` являются типизированными свойствами execution environment, а не новыми демонами.

`luna-system-runtime` координирует:

- system startup;
- системные службы;
- устройства и системные события;
- UserSession;
- lifecycle runtime;
- shutdown/reboot/power transitions;
- состояние системы.

Он не должен превращаться в глобальный mutable object со знанием внутренних деталей всех crate'ов.

---

# 21. UserSession

`UserSession` объединяет identity пользователя и конкретную активную session.

Логика:

```text
luna-system-runtime
        ↓
UserSession
```

UserSession отвечает за:

- lifecycle пользовательской сессии;
- связь authenticated user и session;
- запуск пользовательского application runtime;
- графическое окружение пользователя.

Нет отдельного `luna-session` компонента.

---

# 22. Authentication и login

Штатный login является графическим.

Логика:

```text
system runtime
    ↓
graphical login
    ↓
authentication
    ↓
Active UserSession
```

Нужны отдельные понятия:

- identity — кто пользователь;
- authentication — как доказана личность;
- authorization — какие действия разрешены.

Они не должны сливаться в одну неясную сущность.

---

# 23. Desktop

Штатная среда:

```text
Wayland
  ↓
niri
  ↓
Noctalia Shell
```

Терминал:

```text
Ghostty + fish
```

TTY допустим для development, diagnostics и recovery, но не является штатной точкой пользовательского входа.

Desktop components не являются владельцами immutable system lifecycle.

---

# 24. Device и volume management

`luna-device-manager` отвечает за обнаружение и lifecycle device/volume сущностей.

Целевой пользовательский сценарий для внешнего носителя:

```text
USB inserted
   ↓
device discovered
   ↓
filesystem detected
   ↓
volume mounted
   ↓
desktop notified
   ↓
file manager shows volume
```

Пользователю не требуется вручную выполнять `mount` в терминале.

Подсистема должна учитывать:

- removable media;
- safe unmount/eject;
- permissions;
- filesystem failures;
- device hotplug.

---

# 25. Network / Audio / Bluetooth

Luna имеет отдельные domain boundaries для:

```text
luna-network
luna-audio
luna-bluetooth
```

Эти boundaries не означают обязательное наличие собственных daemon'ов.

Допустимо использовать upstream infrastructure:

```text
NetworkManager
PipeWire / WirePlumber
BlueZ
D-Bus
```

если внешний сервис используется внутри явного Luna contract.

---

# 26. Configuration и State

Конфигурация и durable state разделены.

```text
configuration
    ↓
что пользователь/администратор настроил

state
    ↓
какое устойчивое состояние система имеет сейчас
```

`luna-config` использует TOML там, где человекочитаемая конфигурация уместна.

`luna-state` использует синхронный durable backend `redb`.

State должен поддерживать транзакционные изменения и согласование долгоживущих операций.

---

# 27. Events

`luna-event` задаёт домен событий и контракты доставки.

События должны быть типизированными и ограниченными соответствующей областью.

Примеры концепций:

```text
DeviceAdded
DeviceRemoved
VolumeMounted
UserLoggedIn
UserLoggedOut
ApplicationStarted
ApplicationExited
SystemUpdated
KernelChanged
```

Один глобальный enum не должен становиться свалкой несвязанных событий.

---

# 28. Bundle Format

Приложения и installable components используют Luna Bundle Format.

Транспортное представление:

```text
.lbp
```

Принятый RFC:

```text
RFC-0002 — Bundle Format v1
```

В текущей реализации `luna-bundle` содержит LBP1 codec.

Для LBP1 принятыми являются следующие базовые свойства формата:

- magic/version LBP1;
- 64-байтный header;
- TOML manifest;
- детерминированный TAR payload;
- BLAKE3-256 content identity;
- zstd compression;
- предусмотренный codec/verification boundary для Ed25519.

`.lbp` — это transport/archive representation Bundle, а не установленная runtime-среда.

System Image и Bundle являются разными форматами:

```text
System Image → .squashfs + .toml
Bundle       → .lbp
```

`luna-bundle` не знает о bootloader и не содержит boot logic.

---

# 29. Application Manager

`luna-app-manager` владеет lifecycle установленных Bundles:

```text
inspect
  ↓
validate
  ↓
integrity/trust checks
  ↓
security decision
  ↓
stage
  ↓
atomic commit
```

Он отвечает за:

- install/import;
- verification;
- registration;
- update/removal;
- migration;
- application-data cleanup policy;
- импорт поддерживаемых `.deb`/`.rpm` в Luna Bundle form.

Он не владеет нормальным process execution и не заменяет `luna-app-runtime`.

---

# 30. Application execution architecture

Это одна из центральных архитектурных цепочек Luna.

```text
Bundle declaration
        ↓
ApplicationPlan
        ↓
MappingPlan
        ↓
luna-security
        ↓
luna-namespace
        ↓
luna-app-runtime
        ↓
ApplicationInstance
```

Иерархия владения и security pipeline не должны путаться между собой.

Владение:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

Security pipeline:

```text
Bundle
 ↓
ApplicationPlan
 ↓
MappingPlan
 ↓
Security
 ↓
Namespace materialization
```

---

# 31. ApplicationPlan

`ApplicationPlan` — типизированный план конкретного будущего запуска.

Он должен содержать только данные, необходимые для планирования execution:

- application identity;
- выбранный Bundle;
- executable;
- RuntimeSpec;
- user/session context;
- environment requirements;
- requested permissions/capabilities;
- filesystem requirements;
- resource requirements.

Plan не является permission grant.

---

# 32. MappingPlan

`MappingPlan` описывает filesystem/resource mapping, необходимый приложению.

Он должен строиться из Bundle declaration и runtime/user/system context.

Физические пути DATA не должны утекать в Bundle mapping declarations как публичная семантика.

Например логическое окружение может иметь:

```text
/
├── app
├── lib
├── data
├── tmp
└── ...
```

при том что реальные host mounts совершенно другие.

---

# 33. Security

`luna-security` проверяет, разрешено ли сформированному plan получить запрошенные ресурсы.

Обязательный порядок:

```text
ApplicationPlan
      ↓
MappingPlan
      ↓
SECURITY
      ↓
namespace materialization
```

Security error всегда fail closed.

Запрошенное приложением право не является автоматически выданным правом:

```text
request ≠ grant
```

---

# 34. Root Mapping

`luna-root-mapping` отвечает за логическую модель mappings и построение валидированного MappingPlan.

Он не должен принимать за security policy решения о том, что разрешено.

Это отдельная ответственность:

```text
root-mapping → что потребуется
security     → можно ли это разрешить
namespace    → как материализовать
```

---

# 35. Namespace

`luna-namespace` реализует Linux namespace/materialization primitives.

Для приложений используются существующие kernel mechanisms, прежде всего mount namespaces, bind mounts и Root Mapping.

После materialization приложение получает controlled filesystem view и по умолчанию не видит весь host filesystem.

Namespace layer не является владельцем Bundle policy.

---

# 36. ApplicationInstance

`ApplicationInstance` — конкретный запущенный экземпляр приложения.

Он принадлежит `luna-app-runtime`.

Lifecycle должен быть явным, например:

```text
Created
  ↓
Starting
  ↓
Running
  ↓
Stopping
  ↓
Exited
```

или через `Failed` при неуспешном запуске.

Application runtime failure не должен сам по себе уничтожать unrelated UserSessions.

Runtime должен уметь корректно очищать:

- process tree;
- namespace;
- mounts;
- cgroup/resource state;
- временные данные.

---

# 37. RuntimeKind и RuntimeSpec

`RuntimeKind` — только типизированное свойство execution environment.

Концептуально:

```text
Luna   → native Luna userspace / musl
Glibc  → разрешённая glibc compatibility environment
Bundle → bundle-private runtime, если разрешён политикой
```

Это не отдельные демоны и не уровни `luna-runtime`.

---

# 38. Изоляция зависимостей

Luna предпочитает адресуемые и изолированные версии библиотек вместо одной глобальной конфликтующей директории.

Цель:

```text
Application A → dependency set A
Application B → dependency set B
```

Общие системные библиотеки не должны автоматически означать, что каждое приложение обязано использовать одну mutable глобальную версию.

---

# 39. Update Manager

`luna-update-manager` отвечает за state-changing update transactions.

Для System Image желаемый lifecycle:

```text
download
  ↓
verify
  ↓
stage new image
  ↓
leave current intact
  ↓
activate
  ↓
reboot
  ↓
health confirmation
  ↓
commit / rollback
```

Обновление image не должно переписывать старый image до того, как новая версия признана безопасной.

Kernel update является независимым transaction path.

---

# 40. Rollback и checkpoint

`luna-update-manager` оркестрирует checkpoint/rollback.

`luna-state` хранит durable operation state.

В существующей реализации присутствует checkpointed update/rollback orchestration; интеграция должна продолжать укрепляться health-gated semantics.

Rollback не должен означать удаление пользовательских данных.

---

# 41. Retention

Retention policy должна быть настраиваемой.

Минимальная концепция:

```text
factory
current
previous
older fallback choices
```

Factory обычной очисткой не удаляется.

Удаление старого image/kernel разрешается только тогда, когда это не уничтожает единственный рабочий fallback или необходимую активную сущность.

---

# 42. Recovery

Recovery — отдельный boot environment.

Он должен уметь как минимум:

- просматривать System Images и kernels;
- проверять состояние;
- выбирать rollback;
- переходить в Factory;
- отключать проблемные компоненты;
- выполнять диагностические действия;
- работать при сломанном обычном userspace.

Recovery не является TTY login.

---

# 43. Factory Environment

Factory environment запускает сохранённую заводскую пару:

```text
Factory System Image
+
Factory Kernel
```

Factory является последней штатной известной рабочей точкой перед Recovery.

---

# 44. Диагностика

Полноценная Luna должна иметь структурированную диагностику:

```text
health collection
 ↓
DiagnosticReport
 ↓
bounded repair
 ↓
external export if required
```

Диагностика не должна без ограничений сохранять приватные данные пользователя.

---

# 45. Power management

Полноценная ОС должна поддерживать как минимум:

- shutdown;
- reboot;
- suspend/resume;
- battery/AC state там, где он доступен;
- display hotplug;
- базовые thermal/power transitions.

Конкретный provider stack должен быть определён отдельным контрактом, если existing Linux infrastructure недостаточна.

---

# 46. Network

Для daily-use PC нужны:

- Ethernet;
- Wi-Fi;
- loopback;
- IPv4/IPv6;
- DHCP;
- DNS;
- routing.

В текущем направлении используется NetworkManager как implementation infrastructure, а Luna предоставляет собственную domain boundary и события.

---

# 47. Audio

Для desktop нужны:

- audio devices;
- profiles;
- volume control;
- routing;
- session/user integration.

PipeWire/WirePlumber могут использоваться как implementation infrastructure внутри соответствующей Luna boundary.

---

# 48. Bluetooth

Нужны:

- discovery;
- pairing;
- trust state;
- device lifecycle;
- authorization.

BlueZ является допустимым implementation provider внутри Luna Bluetooth boundary.

---

# 49. File manager и file access

Внешний file manager является пользовательским клиентом, а не владельцем storage policy.

Он должен работать с:

- logical user data;
- volumes;
- application file operations;
- errors/permissions;
- safe external media lifecycle.

Отдельная будущая архитектурная область — file access/portal model. Приложение не должно получать полный доступ ко всему home только потому, что пользователю нужно открыть один файл.

---

# 50. Installer

Полноценная Luna требует installer/initial provisioning path:

```text
boot installation media
 ↓
select disk
 ↓
provision EFI/SYSTEM/DATA/SWAP
 ↓
install luna-boot
 ↓
install factory System Image
 ↓
install factory kernel
 ↓
create initial state
 ↓
create first user/admin
 ↓
reboot
```

Installer не является частью обычного runtime и разрабатывается как отдельная release/installability subsystem.

---

# 51. Security foundation

Используем существующие Linux mechanisms, а Luna формирует из них контролируемую policy model.

Relevant primitives:

```text
uid/gid
capabilities
mount namespaces
other namespaces where needed
seccomp where required
cgroups/resource controls
```

Принцип Luna:

> чем ближе ресурс к host/system boundary, тем более явным должно быть разрешение.

---

# 52. IPC

Полноценной ОС нужен IPC между:

- system runtime и services;
- system runtime и UserSession;
- UserSession и application runtime;
- applications и разрешёнными system services.

Не следует заранее создавать универсальный Luna RPC для всего проекта. Сначала определяются реальные API contracts, затем выбирается транспорт.

Допустимые Linux primitives включают Unix sockets, pipes, eventfd, shared memory и netlink в зависимости от задачи.

---

# 53. Common crate

`luna-common` содержит только маленькие действительно общие value types, identifiers и фундаментальные типы.

Он не должен превращаться в repository-wide свалку.

Если тип относится только к Bundle, он принадлежит `luna-bundle`. Если только к boot — он не должен автоматически попадать в `luna-common`.

---

# 54. Crate map

Текущие workspace boundaries:

| Компонент | Ответственность |
|---|---|
| `luna-common` | маленькие общие идентификаторы и value types |
| `luna-fs` | низкоуровневые filesystem primitives и metadata |
| `luna-root-mapping` | логика Root Mapping и MappingPlan |
| `luna-namespace` | Linux namespaces/materialization |
| `luna-config` | конфигурация и области конфигурации |
| `luna-security` | policy, authorization, grants и trust boundary |
| `luna-state` | durable state, storage abstraction и transactions |
| `luna-event` | events, subscriptions и delivery contracts |
| `luna-bundle` | Bundle domain и RFC-0002/LBP1 codec |
| `luna-app-manager` | install/import/update/removal/verification/migration |
| `luna-system-manager` | system state model и queries |
| `luna-update-manager` | update transactions, checkpoints и rollback |
| `luna-kernel-manager` | kernel inventory, metadata, compatibility |
| `luna-device-manager` | device/volume discovery и lifecycle |
| `luna-system-runtime` | system-wide supervision и UserSession orchestration |
| `luna-user-session` | UserSession domain и lifecycle contract |
| `luna-app-runtime` | ApplicationInstance execution/lifecycle |
| `luna-login` | graphical login boundary |
| `luna-cli` | thin user-facing CLI |
| `luna-files` | file-manager client/boundary |
| `luna-audio` | audio domain/provider boundary |
| `luna-network` | network domain/provider boundary |
| `luna-bluetooth` | Bluetooth domain/provider boundary |
| `luna-init` | standalone musl early userspace |

Отдельно:

```text
boot/luna-boot/
└── luna-boot.efi
```

`luna-boot.efi` находится вне ordinary userspace workspace.

---

# 55. Dependency direction

Нижние слои не должны зависеть от верхних ради удобства.

Концептуально:

```text
luna-common
    ↑
foundation
    ↑
policy/state/bundle/domain
    ↑
managers
    ↑
runtime
    ↑
CLI / GUI clients
```

Это логическое направление, а не разрешение на произвольные циклические зависимости.

---

# 56. Граница внешних implementation components

Следующие вещи являются implementation infrastructure, а не автоматически новыми Luna component boundaries:

- NetworkManager;
- PipeWire/WirePlumber;
- BlueZ;
- D-Bus;
- greetd/greeter;
- niri-session wrappers;
- setpriv и подобные identity helpers;
- Yazi;
- другие upstream userspace providers.

Их использование допустимо только внутри явной Luna boundary.

---

# 57. Definition of Done для полноценной ОС

Luna можно считать полноценной PC operating system только когда имеется исполняемая интеграция:

```text
install
 ↓
UEFI boot
 ↓
System Image + compatible kernel
 ↓
luna-init
 ↓
logical /
 ↓
system runtime
 ↓
graphical authentication
 ↓
UserSession
 ↓
niri + Noctalia
 ↓
applications
 ↓
files / network / audio / Bluetooth / removable media
 ↓
update + rollback + recovery
 ↓
shutdown / reboot / resume
```

Наличие placeholder, TOML-файла, пустого crate или документа не считается доказательством готовности интеграции.

---

# 58. Текущий статус проекта

На момент редакции Source of Truth:

- архитектурный цикл Phase 1.1–1.6-HZ завершён и решения консолидированы;
- проект находится в Phase 2: runtime/boot integration, PC bring-up, desktop integration и hardening;
- `luna-namespace` содержит рабочие Linux namespace/materialization primitives;
- `luna-state` использует `redb` как первый durable backend;
- `luna-update-manager` содержит checkpointed update/rollback orchestration;
- `luna-bundle` содержит LBP1 implementation для RFC-0002;
- `luna-system-runtime` уже работает с реальными Linux child processes и UserSession/process lifecycle;
- `luna-app-runtime` владеет ApplicationInstance lifecycle и execution setup;
- `luna-init` реализован как standalone musl early-userspace binary;
- существует reproducible x86_64 UEFI/GPT PC development image и QEMU/OVMF bring-up path;
- desktop payload уже включает native niri/Noctalia stack, graphical login, Ghostty, fish, Yazi, audio, network, Bluetooth и removable-media service payload;
- hardware validation seat/input/GPU и часть production integration ещё остаются работой.

Этот статус не меняет архитектурных контрактов и не означает, что перечисленный компонент полностью завершён.

---

# 59. Phase 0 — архитектурные контракты

Перед новым крупным implementation work должны быть доведены до согласованного состояния пять контрактов:

```text
System Image
Kernel
Boot State
Boot Handoff
Failure / Recovery
```

Текущие документы на ветке `develop` имеют статус **черновик**:

```text
docs/contracts/SYSTEM-IMAGE-CONTRACT.md
docs/contracts/KERNEL-CONTRACT.md
docs/contracts/BOOT-STATE-CONTRACT.md
docs/contracts/BOOT-HANDOFF-CONTRACT.md
docs/contracts/FAILURE-RECOVERY-CONTRACT.md
```

Их нельзя считать принятыми архитектурными решениями до отдельного рассмотрения.

---

# 60. План дальнейшей разработки

Полный рабочий план находится в:

```text
docs/architecture/DEVELOPMENT-ROADMAP.md
```

Последовательность:

```text
Phase 0  → контракты
Phase 1  → UEFI / luna-boot
Phase 2  → luna-init / logical root
Phase 3  → system runtime / state / events
Phase 4  → devices / storage
Phase 5  → user / authentication / graphical session
Phase 6  → bundles / application manager
Phase 7  → security / mapping / namespace / app runtime
Phase 8  → updates / kernel / rollback / recovery
Phase 9  → production hardware
Phase 10 → installer / release engineering
```

---

# 61. Правило реализации

Перед написанием существенного Rust-кода необходимо определить:

1. типы данных и их границы;
2. кто владеет данными;
3. где используются `&T` и `&mut T`;
4. какие операции возвращают `Result<T, E>`;
5. где отсутствие — это `Option<T>`;
6. какие ошибки являются ожидаемыми operational failures;
7. где находится crate/module boundary;
8. какие инварианты должны быть подтверждены тестами.

В коде не следует использовать `panic!` для обычных отказов устройств, файлов, прав или внешних ресурсов. Неожидаемое нарушение внутреннего invariant может быть panic, но не штатная operational ошибка.

---

# 62. Жёсткие архитектурные запреты

Нельзя:

- создавать generic `luna-runtime`;
- помещать Bundle Format logic в `luna-fs`;
- добавлять bootloader knowledge в `luna-bundle`;
- материализовывать namespace до security decision;
- выдавать requested capabilities как automatic grants;
- делать DATA частью immutable System Image;
- связывать update System Image и kernel в одну обязательную единицу;
- удалять factory ordinary retention policy;
- переписывать boot state на каждом обычном boot;
- считать наличие файла доказательством совместимости;
- создавать новый crate только ради будущей идеи без реальной разработки.

---

# 63. Что ещё требует отдельной спецификации

Пока не считаются окончательно закрытыми:

- точная TOML-схема System Image manifest;
- точная kernel metadata schema;
- физический формат Boot State;
- точные критерии health confirmation;
- окончательная failure state machine;
- точная retention policy;
- filesystem/root mapping details для каждого execution mode;
- file portal model;
- authentication provider boundary;
- service manager integration;
- device/volume backend contract;
- firmware lifecycle;
- signing/trust/repository policy для System Images;
- installer contract.

Ни одно из этих решений нельзя молча считать уже принятым только потому, что оно удобно реализации.

---

# 64. Основная архитектурная цепочка Luna

В сжатом виде вся система должна сохранять следующие границы:

```text
UEFI
  ↓
luna-boot.efi
  ↓
compatible kernel + System Image selection
  ↓
Linux kernel
  ↓
luna-init
  ↓
logical /
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
luna-app-runtime
  ↓
ApplicationInstance
```

А внутри запуска приложения:

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

Это архитектурный позвоночник Project Luna.