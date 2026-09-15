# Project Luna — Источник истины

**Статус:** нормативный документ
**Редакция:** 2026-09-15

Этот документ — главный архитектурный источник истины Project Luna. Актуальные документы, на которые он ссылается, являются единственными рабочими описаниями архитектуры. Старые варианты не используются как нормативные источники.

## 1. Главные инварианты

Project Luna — собственная архитектура ОС поверх ядро Linux.

Каноническая нормальная цепочка:

```text
UEFI
  ↓
luna-boot.efi
  ↓
ядро Linux
  ↓
luna-init (PID 1)
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
luna-app-runtime
  ↓
ApplicationInstance
```

`luna-init` — первый процесс пользовательского пространства и PID 1. Он выполняет раннюю первичную инициализацию, остаётся PID 1 и запускает `luna-system-runtime` как дочерний общесистемный runtime.

В архитектуре нет `luna-core`, generic `luna-runtime`, `luna-session`, `luna-run-session` или `luna-app-init`.

Нормальная загрузка не использует initramfs, `switch_root` или `pivot_root`.

## 2. Физическая модель диска

```text
Disk
├── EFI
├── LUNA-SYS
├── LUNA-DATA
└── SWAP
```

Имена `LUNA-SYS` и `LUNA-DATA` — канонические имена разделов. Внутри архитектуры нельзя возвращаться к безымянной паре `SYSTEM`/`DATA` там, где речь идёт о физических разделах.

`EFI` и `LUNA-SYS` образуют единую пару для загрузки и должны находиться на одном физическом диске. `luna-boot.efi` проверяет это условие и загружает ОС только из `LUNA-SYS` того же диска. `LUNA-DATA` может находиться на том же диске или на втором физическом диске. `SWAP` отделён от Luna-разделов. `LUNA-SYS` — управляемый ОС и недоступен обычному пользователю; `LUNA-DATA` — постоянная изменяемая область.
## 3. LUNA-SYS

Каноническая структура физического раздела:

```text
LUNA-SYS/
├── images/
│   ├── luna-X.Y.Z.squashfs
│   ├── luna-X.Y.Z.toml
│   └── ...
├── cores/
│   ├── luna-X.Y.Z.init
│   ├── luna-X.Y.Z.toml
│   └── ...
├── kernels/
│   └── <kernel-id>/
│       └── ...
├── config/
│   ├── boot-state.toml
│   ├── luna-data.toml
│   └── ...
└── recovery/
    ├── recovery-X.Y.Z.squashfs
    ├── recovery-X.Y.Z.toml
    └── ...
```

`images/` содержит System Images и их соседние manifests. System Image — непосредственно `luna-X.Y.Z.squashfs`. Внутри System Image находится минимальная неизменяемая база ОС: системные приложения, необходимые библиотеки, драйверы, отдельный каталог firmware, configuration defaults и неизменяемые ресурсы. `drivers/` и `firmware/` — разные классы ресурсов и не объединяются в один каталог. Внешние upstream-компоненты, необходимые для работы ОС, могут входить в этот payload, но они не становятся принадлежащий Luna архитектурными компонентами только потому, что поставляются вместе с System Image.

`cores/` содержит самостоятельные версионируемые ELF-артефакты `luna-init` и их manifests. `.init` не является System Image, Bundle или образ файловой системы.

`kernels/` содержит независимые версионируемые ядра Linux. `config/` содержит boot state и `luna-data.toml`. Этот манифест хранит GUID физического диска и GUID раздела целевого `LUNA-DATA` для быстрого подключения. `recovery/` содержит versioned Recovery DATA Images и их manifests; каноническая форма Recovery DATA Image — `recovery-X.Y.Z.squashfs` рядом с `recovery-X.Y.Z.toml`. Отдельного Recovery System Image нет.

## 4. LUNA-DATA

```text
LUNA-DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── firmware/
│   ├── libs/
│   ├── config/
│   ├── state/
│   ├── volumes/
│   └── ...
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

`system/` содержит управляемый ОС изменяемые расширения и состояние; `users/` содержит пользовательские данные; `cache/` не является durable storage.

Физический путь раздела не является application-facing API. Приложения получают только разрешённое логическое представление файловой системы.

## 5. System Image

System Image — неизменяемая, версионированная минимальная userspace-база конкретного релиза Luna. Это источник файловой системы, из которого `luna-init` материализует работающую системную среду в логическом `/` на основе RAM. Внутри image находится минимально необходимый набор системных приложений Luna, библиотек, драйверов, конфигурации по умолчанию и неизменяемых ресурсов для полноценного запуска ОС. Дополнительные и изменяемые возможности системы предоставляются через `LUNA-DATA/system`. Канонический payload:

```text
LUNA-SYS/images/luna-X.Y.Z.squashfs
LUNA-SYS/images/luna-X.Y.Z.toml
```

`.squashfs` является непосредственным образ файловой системы. `.lbp` не является System Image и не содержит System Image как свой формат runtime.

System Image — неизменяемый источник. Он не является постоянный логический `/` и не копируется целиком в RAM. `luna-init` использует его для материализации критически важные для загрузки ресурсы и может гидратировать дополнительные неизменяемые ресурсы лениво. Пользовательские данные, изменяемое состояние системы, установленные расширения системы и cache находятся вне него. Структура image повторяет основные системные классы ресурсов `LUNA-DATA/system`, но содержит только их минимальную неизменяемая база: `apps/`, `drivers/`, `libs/`, `config/`, а также `resources/` и необходимый `firmware/`. Долговечный `state/` и управляемый `volumes/` относятся к изменяемым DATA и не являются частью immutable System Image.
## 6. Двухэтапная совместимость артефакты загрузки

System Image, `luna-init` и ядро независимы, но целевой объект загрузки собирается только через две последовательные проверки:

```text
System Image
    │
    │ manifest image: совместимые `luna-init`
    ↓
luna-init core
    │
    │ init manifest: совместимые ядра
    ↓
ядро Linux
```

Манифест System Image определяет допустимые версии `luna-init`. Манифест `luna-init` определяет допустимые kernels. Манифест `luna-init` не определяет совместимость с System Image.

Полный целевой объект загрузки имеет вид `System Image + luna-init + kernel`. `luna-boot.efi` формирует эту тройку через цепочку `image → init → kernel`, после чего загружает kernel и выбранный `.init`; ядро Linux запускает `luna-init` как PID 1, а `luna-init` материализует выбранный System Image.

## 7. Current, Factory и Recovery

`current` — подтверждённый текущий полный целевой объект загрузки `System Image + luna-init + kernel`.

`factory` — сохранённый заводской полный целевой объект загрузки `System Image + luna-init + kernel`.

`recovery` — полный Recovery target: System Image + luna-init + kernel + Recovery DATA Image. Recovery использует обычный System Image как системный source; отдельным recovery-артефактом является только Recovery DATA Image, который материализуется в RAM как виртуальная `luna-data` среда.

```text
current  = System Image + luna-init + kernel
factory  = System Image + luna-init + kernel
recovery = System Image + luna-init + kernel + Recovery DATA Image
```

При фактической загрузке для каждой пары выполняется двухэтапное разрешение `image → init → kernel`. `luna-init` является реальным членом полного target и не является опциональным промежуточным артефактом.

## 8. luna-boot.efi

`luna-boot.efi` — самостоятельный UEFI загрузчик и владелец решений до передачи управления ядро Linux. Он проверяет, что EFI и `LUNA-SYS` находятся на одном физическом диске, использует `LUNA-SYS` именно с этого диска, читает boot state и `luna-data.toml`, разрешает `LUNA-DATA`, выполняет discovery/разрешение совместимости, Boot Menu, загрузку kernel/init, подготовку memory-resident передачу handoff и `ExitBootServices`.

Он не является userspace runtime, не управляет UserSession или приложениями и не создаёт дополнительные слои runtime. Нормальная загрузка не показывает меню и не добавляет искусственную задержку; `B` — явное исключение.

`luna-boot.efi` использует:

```text
LUNA-SYS/images/
LUNA-SYS/cores/
LUNA-SYS/kernels/
LUNA-SYS/config/
LUNA-SYS/recovery/
```

Решение целевой объект загрузки строится через `image → init → kernel`, а не через независимый выбор каждого артефакта.

## 9. Выбор загрузки

`luna-boot.efi` сканирует `LUNA-SYS/images`, `LUNA-SYS/cores`, `LUNA-SYS/kernels`, `LUNA-SYS/config` и `LUNA-SYS/recovery`. Для DATA он сначала использует GUID-пару из `LUNA-SYS/config/luna-data.toml`; если привязанный диск или раздел недоступен, обычная загрузка переходит в Recovery.

Для нормального загрузчик target:

1. выбирает System Image по политике загрузки;
2. читает его manifest;
3. получает только совместимые init cores;
4. для каждого допустимого init читает его manifest;
5. оставляет только kernels, объявленные совместимыми этим init;
6. формирует полный target `image + init + kernel`.

При ручном выборе System Image пользователь видит совместимые init cores и для выбранного init — только совместимые kernels. Несовместимые сочетания не показываются и не запускаются.

## 10. Прямой запуск initial userspace

`luna-boot.efi` загружает выбранный `.init` ELF в зарезервированную для загрузки физическую память и передаёт его идентичность через `LunaBootHandoffV1`/Linux `setup_data`.

ядро Linux повторно проверяет передачу handoff, диапазон памяти, дайджест и ограничения ELF, создаёт внутренний объект исполняемого файла, размещённый в памяти ядра и использует существующую механизм выполнения Linux ELF/binfmt.

После инициализации ядра первым процессом пользовательского пространства становится `luna-init` и получает PID 1. Никакого initramfs, temporary `/init`, `switch_root` или `pivot_root` для нормальной загрузки нет.

`luna-init` получает read-only контекст загрузки через FD 3, выполняет ранний первичную инициализацию, подготавливает необходимые системные ресурсы и остаётся PID 1. После подготовки он запускает `luna-system-runtime` как дочерний процесс.

## 11. Boot state и отказы

`LUNA-SYS/config/boot-state.toml` хранит долгоживший контекст загрузки и атомарные target (`current`, `fallback`, `recovery`, `factory`). Эти targets не переписываются на каждой загрузке и изменяются только при значимых значимых постоянных изменений системы; их mutation принадлежит `luna-update-manager`.

`LunaBootAttempt` — отдельный минимальный persistent marker в UEFI NVRAM. `luna-boot.efi` записывает `in_progress` один раз после подготовки target/kernel/init/передачу handoff и непосредственно перед `ExitBootServices`. После `ExitBootServices` загрузчик больше не управляет marker.

Подробный `BootAttemptProgress` хранится только в RAM текущего запуска и проходит монотонные стадии от `BootloaderLoaded` до `Success`. Наличие `in_progress` при следующем запуске означает только, что предыдущая попытка не достигла `SUCCESS`; сам marker не определяет причину.

Финальную semantic success confirmation выполняет `luna-system-runtime` после инициализации runtime и system state; при успехе он очищает `LunaBootAttempt` через `efivarfs`.

Если загруженный kernel остаётся работоспособным, failure System Image или early userspace может выполнить soft fallback на другой image, совместимый с тем же уже загруженным kernel. При kernel panic требуется reboot; следующий `luna-boot.efi` выбирает предыдущий совместимый ядро/target.
## 12. Владение runtime

```text
luna-init (PID 1)
├── luna-system-runtime
│   ├── system services
│   └── UserSession
│       └── luna-app-runtime
│           └── ApplicationInstance
```

`luna-init` владеет ранним первичную инициализацию и PID 1 boundary, но остаётся PID 1. `luna-system-runtime` владеет долговременный системной supervision. `UserSession` — доменная сущность внутри system runtime, не отдельный daemon.

`luna-app-runtime` владеет жизненным циклом запуска приложений и состоянием `ApplicationInstance`. Внутри экземпляра используются `ApplicationPlan`, `MappingPlan`, `luna-root-mapping`, `luna-security` и `luna-namespace`.

## 13. Выполнение приложений

`luna-app-runtime` строит один `ApplicationInstance` для конкретного запрос запуска. Внутри его жизненный цикл выполняется:

```text
plan
  ↓
mapping
  ↓
authorization
  ↓
материализацию
```

```text
ApplicationPlan
    ↓
MappingPlan
    ↓
luna-root-mapping
    ↓
luna-security
    ↓
AuthorizedApplicationPlan
    ↓
luna-namespace
    ↓
ApplicationInstance
    ↓
process
```

`ApplicationInstance` содержит `RuntimeSpec`, определяющий используемую libc: `musl` или `glibc`. Один процесс использует одну libc. `RuntimeSpec` является частью описания среды выполнения, а не самостоятельным компонентом. Authorization предшествует материализацию; `request != grant`. Изоляция через mount namespace обязательна; PID namespace по умолчанию не создаётся. Физические пути `LUNA-SYS`/`LUNA-DATA` не являются частью контракта приложения.

## 14. Recovery и Factory

Recovery и Factory — целевой объект загрузкиs, а не отдельные слои runtime. Recovery использует обычный System Image, совместимый `luna-init` и kernel, а также отдельный Recovery DATA Image, материализованный в RAM как `VirtualData`. Recovery DATA Image содержит схему `luna-data` для виртуального recovery-пользователя и программное окружение для диагностики и восстановления системы. Recovery содержит инструмент поиска и выбора физической `LUNA-DATA`; при нескольких валидных кандидатах пользователь выбирает нужный, после чего Recovery может записать выбранные GUID диска/раздела в `LUNA-SYS/config/luna-data.toml`.

Factory использует сохранённый factory `System Image + luna-init + kernel` target и обычную физическую `LUNA-DATA`.

## 15. Модель Bundle

`.lbp` — transport/archive representation Luna Bundle Format, определённый RFC-0002. Bundle и System Image — разные форматы:

```text
System Image → .squashfs + соседний `.toml`
luna-init    → .init + соседний `.toml`
Bundle       → .lbp
```

## 16. Исходные документы

| Area | Canonical document |
|---|---|
| Disk layout | [`architecture/DISK-LAYOUT.md`](architecture/DISK-LAYOUT.md) |
| Complete boot path | [`architecture/BOOT-PATH.md`](architecture/BOOT-PATH.md) |
| Boot state | [`architecture/BOOT-STATE.md`](architecture/BOOT-STATE.md) |
| Recovery / Factory | [`architecture/RECOVERY.md`](architecture/RECOVERY.md) |
| System Image | [`architecture/SYSTEM-IMAGE.md`](architecture/SYSTEM-IMAGE.md) |
| Logical root | [`architecture/LOGICAL-ROOT.md`](architecture/LOGICAL-ROOT.md) |
| Выполнение приложения | [`architecture/APPLICATION-EXECUTION.md`](architecture/APPLICATION-EXECUTION.md) |
| Runtime жизненный цикл | [`architecture/RUNTIME-LIFECYCLE.md`](architecture/RUNTIME-LIFECYCLE.md) |
| Security model | [`architecture/SECURITY-MODEL.md`](architecture/SECURITY-MODEL.md) |
| Update жизненный цикл | [`architecture/UPDATE-LIFECYCLE.md`](architecture/UPDATE-LIFECYCLE.md) |
| Component map | [`architecture/COMPONENT-MAP.md`](architecture/COMPONENT-MAP.md) |
| Component architecture | [`architecture/components/README.md`](architecture/components/README.md) |
| Accepted decisions summary | [`architecture/ACCEPTED-DECISIONS.md`](architecture/ACCEPTED-DECISIONS.md) |

## 17. Правила разработки с участием AI

Эти правила являются нормативными для разработки с участием ассистента.

1. Всегда работать по этому SoT и связанной с ним текущей архитектуре.
2. Не придумывать новые слои, компоненты, daemons, managers или abstractions.
3. Если для работоспособности требуется неописанная часть архитектуры, сначала объяснить необходимость, предложить обоснованные варианты и ждать явного одобрения или отказа.
4. Архитектуру изменять только после обсуждения и явного согласия пользователя. После согласия сначала обновить SoT/contract, затем код.
5. Работать только в предложенной или явно указанной ветке. Не создавать новую ветку ради каждого исправления.
6. Работать только над указанным компонентом. Другие компоненты не менять без запроса или утверждённого contract change.
7. Не выдавать незавершённую реализацию за завершённую архитектурную функциональность.
8. При конфликте кода и SoT не подменять архитектуру случайным поведением кода: сообщить о расхождении и решить его явно.
9. Активная документация должна содержать одну текущую трактовку каждого архитектурного понятия. Старые варианты хранятся вне активного дерева только как справочный архив.

## 18. Иерархия источников

```text
User-approved architecture
        ↓
docs/ARCHITECTURE.md
        ↓
связанные архитектурные документы
        ↓
normative contracts
        ↓
implementation status
        ↓
development notes
        ↓
архив (только справочный)
```

Архив не является источником решений. Если архив содержит полезную информацию, она сначала проверяется на соответствие текущему SoT, после чего нужная информация переносится в актуальный документ.
