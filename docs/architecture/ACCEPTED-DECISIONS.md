# Project Luna — Актуальные принятые решения

**Статус:** нормативная сводка принятых архитектурных решений
**Ревизия:** 2026-09-18
**Источник авторитета:** `docs/ARCHITECTURE.md`

Этот документ объединяет принятые решения, затрагивающие несколько архитектурных областей. Это текущая сводка, а не исторический журнал. Если архивный текст противоречит этому документу или `docs/ARCHITECTURE.md`, действуют текущие документы.

## 1. Основа Project Luna

- Имя проекта: **Project Luna**; внутреннее имя `luna`.
- Язык реализации: Rust.
- Лицензия проекта: Apache License 2.0.
- Linux используется как основа ядра; Luna определяет собственную архитектуру ОС поверх него.
- One File Linux является архитектурным источником вдохновения, а не ограничением реализации.
- Основа системы намеренно мала, стабильна и преимущественно неизменяема.
- Существующие механизмы Linux предпочтительны там, где они дают подходящий базовый примитив.
- Новые архитектурные компоненты или crates требуют явно определённой границы ответственности и одобрения.

## 2. Физическое хранилище

Каноническая модель разделов:

```text
Disk
├── EFI
├── LUNA-SYS
├── LUNA-DATA
└── SWAP
```

- `LUNA-SYS` управляется ОС и скрыт от обычного пользователя.
- `LUNA-DATA` — граница постоянных изменяемых данных.
- `EFI` содержит UEFI-инфраструктуру загрузки.
- `SWAP` отделён; конкретный механизм может быть разделом, файлом или другим согласованным способом.
- EFI и `LUNA-SYS` должны находиться на одном физическом диске; `luna-boot.efi` проверяет это и загружает ОС из `LUNA-SYS` этого диска.
- `LUNA-DATA` может находиться на том же физическом диске или на другом.
- `LUNA-SYS/config/luna-data.toml` хранит GUID целевого диска DATA и GUID раздела для быстрого подключения. При недоступной или неоднозначной привязке Recovery выполняет явный поиск и выбор DATA и может сохранить выбранную пару GUID.
- Точная политика разметки установщика остаётся отдельной задачей спецификации и реализации.

## 3. LUNA-SYS и System Image

Канонический системный раздел содержит:

```text
LUNA-SYS/
├── images/
├── cores/
├── kernels/
├── config/
└── recovery/
```

- Обычный System Image — непосредственно файловая система SquashFS: `luna-X.Y.Z.squashfs`.
- Его соседний манифест: `luna-X.Y.Z.toml`.
- `.lbp` никогда не является форматом System Image.
- Глобального манифеста System Image не существует.
- System Image — не вся ОС, а неизменяемый версионированный минимальный источник userspace, из которого `luna-init` материализует работающую системную среду. Он должен содержать immutable minimum, достаточный для полноценного запуска Luna даже при отсутствии физической `LUNA-DATA`.
- `LUNA-DATA` — отдельная изменяемая и расширяемая часть ОС; она добавляет persistent state, Bundles, дополнительные системные компоненты и управляемые изменения поверх immutable base.
- System Images неизменяемы, версионируются и хранятся согласно политике удержания.
- System Image, `luna-init` и kernel версионируются независимо и образуют один полный boot target при корректной цепочке совместимости.
- Точная грамматика манифеста System Image определяется контрактом System Image.

Внутренняя структура System Image:

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
    ├── sounds/
    ├── locales/
    └── translations/
```

- `drivers/` и `firmware/` — отдельные классы ресурсов; firmware никогда не рассматривается как часть каталога drivers.
- Это неизменяемая минимальная системная база. `state/` и `volumes/` относятся к изменяемым данным DATA и не входят в System Image.
- Image содержит минимальную функциональность, необходимую для запуска Luna; `LUNA-DATA/system` расширяет её дополнительными компонентами, изменяемой конфигурацией/состоянием и управляемыми ресурсами.
- Внешние providers, используемые Luna, включая Wayland, niri, Noctalia Shell, Ghostty, fish, PipeWire/WirePlumber, BlueZ, NetworkManager и Yazi, остаются внешним программным обеспечением даже при включении их файлов в System Image.
- Luna-owned границы компонентов, интегрирующие такие providers, остаются компонентами Luna.

## 4. Версионируемый `luna-init`

`luna-init` — отдельный версионируемый boot-артефакт:

```text
LUNA-SYS/cores/luna-X.Y.Z.init
LUNA-SYS/cores/luna-X.Y.Z.toml
```

- `.init` — исполняемый ELF64-артефакт, а не image или Bundle.
- Его манифест объявляет совместимые kernel identities.
- Манифест System Image объявляет совместимые версии `luna-init`.
- Эти две связи совместимости намеренно разделены.
- Bootloader разрешает `image → init → kernel` и не смешивает артефакты независимо друг от друга.

## 5. Нормальная загрузка и PID 1

Каноническая цепочка:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
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

- `luna-init` остаётся PID 1 на протяжении нормальной работы системы.
- `luna-system-runtime` является дочерним процессом `luna-init`, но никогда не является PID 1.
- `luna-system-runtime` не должен заменять `luna-init` и не должен делать `exec` поверх него.
- TTY/serial shell предназначен только для диагностики, Recovery и разработки, а не для обычного входа пользователя.

## 6. Прямой запуск initial userspace

- `luna-boot.efi` загружает точные байты `.init` в зарезервированную память загрузки.
- `LunaBootHandoffV1` передаёт физический адрес init, размер и digest BLAKE3-256.
- Linux kernel проверяет handoff, диапазон памяти, digest и ограничения ELF.
- Kernel создаёт только внутренний memory-backed executable object, чтобы повторно использовать Linux ELF/binfmt machinery.
- Начальный процесс получает проверенный handoff как read-only FD 3 с offset zero.
- Для запуска `luna-init` не требуется pathname в userspace filesystem.
- Сбой прямого пути является ошибкой загрузки; он не должен молча переходить к `/init`, `/sbin/init` или `/bin/sh`.

## 7. Выбор загрузки, current и factory

- `current` — подтверждённый полный boot target `System Image + luna-init + kernel`.
- `factory` — сохранённый заводской полный boot target `System Image + luna-init + kernel`.
- `luna-init` является реальным членом boot target.
- При загрузке `luna-boot.efi` разрешает `System Image → совместимый luna-init → совместимый kernel` и загружает выбранный kernel и `.init`.
- Kernel запускает `luna-init` как PID 1, а `luna-init` материализует выбранный System Image в RAM-backed logical root.
- Обычный кандидат выбирается по boot policy; приоритет может иметь валидный кандидат с наибольшей версией.
- Factory не является универсальным аварийным shell и не является дополнительным runtime layer.

## 8. Boot state и ошибки

- `LUNA-SYS/config/boot-state.toml` — долговременный контекст boot target, а не журнал live-стадий.
- `LunaBootAttempt` — минимальный marker обнаружения незавершённой загрузки в UEFI NVRAM.
- Он записывается один раз непосредственно перед `ExitBootServices` и очищается только после подтверждения semantic boot success.
- Подробные стадии попытки остаются в RAM.
- Обычная успешная загрузка не должна переписывать durable boot state только потому, что машина запустилась.
- Значимые переходы target/failure/confirmation/recovery могут менять постоянное состояние.
- Сбой System Image или раннего userspace может выполнить soft fallback без reboot, если уже загруженный kernel остаётся работоспособным.
- Kernel panic обрабатывается после reboot выбором предыдущего совместимого kernel/target.

## 9. Recovery

Recovery является полным target:

```text
System Image
+ luna-init
+ kernel
+ Recovery DATA Image
```

- Recovery не имеет отдельного System Image; используется обычный System Image плюс совместимые `luna-init` и kernel.
- Recovery DATA материализуется в RAM как `VirtualData`.
- Recovery DATA Image содержит схему `luna-data` для виртуального recovery-пользователя и программное окружение для диагностики и восстановления системы.
- Recovery может запускаться без физического `LUNA-DATA`.
- Физический DATA может проверяться и восстанавливаться из Recovery, но не является хранилищем работающей Recovery-среды.
- Recovery-специфические data artifacts находятся в единственном `LUNA-SYS/recovery/recovery.squashfs` с `recovery.toml`; Recovery GUI provider — Niri.
- Recovery и Factory — разные режимы и не должны сводиться к generic fallback runtime.

## 10. Модель LUNA-DATA

Каноническая структура постоянного DATA:

```text
LUNA-DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── firmware/
│   ├── libs/
│   ├── config/
│   ├── resources/
│   │   ├── fonts/
│   │   ├── icons/
│   │   ├── themes/
│   │   ├── cursors/
│   │   ├── sounds/
│   │   ├── locales/
│   │   └── translations/
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

- `system/apps` хранит установленные application Bundles.
- `system/drivers` и `system/firmware` — отдельные системно управляемые области ресурсов.
- `system/libs`, `system/config`, `system/resources`, `system/state` и `system/volumes` также являются системно управляемыми областями. `system/resources` повторяет классификацию неизменяемых ресурсов System Image: `fonts`, `icons`, `themes`, `cursors`, `sounds`, `locales`, `translations`. `state` и `volumes` являются mutable-only областями и не входят в System Image.
- `users/<user>/home`, `data` и `config` относятся к конкретному пользователю.
- `cache` является удаляемым и не является авторитетным durable state store.
- Физические пути — детали хранения, а не API приложения.

## 11. Logical root и mapping

- Физические разделы являются источниками хранения, а не application-facing Linux `/`.
- Работающий logical root является RAM-backed и собирается из разрешённых источников.
- Содержимое System Image неизменяемо; изменяемое состояние поступает из разрешённых DATA providers.
- `/dev`, `/proc`, `/sys`, `/run` и `/tmp` являются ресурсами, создаваемыми runtime.
- Boot-critical ресурсы материализуются достаточно рано для запуска; дополнительные неизменяемые ресурсы могут загружаться лениво.
- Материализованные ресурсы должны оставаться пригодными без зависимости от дальнейшей pathname visibility source image.
- `luna-root-mapping` владеет семантикой логического mapping и создаёт проверенные mapping plans.
- Mapping tables являются namespace-local, а не единой глобальной таблицей для всех приложений.
- Гранулярность mapping в основном определяется требуемым файлом; subtree mapping применяется там, где это семантически оправдано.
- Mapping declarations не являются security grants.

## 12. Модель приложения

- Приложения являются неизменяемыми Bundles, подобными модели macOS.
- Установленные Bundles находятся в `LUNA-DATA/system/apps` и доступны нескольким пользователям.
- Разные версии Bundle могут сосуществовать независимо.
- Изменяемые данные приложения и пользовательская конфигурация находятся вне установленного immutable Bundle.
- Пользовательские данные приложения находятся в `LUNA-DATA/users/<user>/data`.
- Пользовательская конфигурация приложения находится в `LUNA-DATA/users/<user>/config`.
- Portable Bundles со съёмного или другого хранилища могут использоваться после проверки целостности, trust и authorization.
- Удаление Bundle само по себе не означает удаление пользовательских данных.
- Осиротевшие данные могут быть обнаружены и намеренно очищены политикой управления приложениями.

## 13. Запуск приложений и безопасность

Канонический поток:

```text
запрос запуска
  ↓
luna-app-runtime
  ↓
ApplicationInstance
  ↓
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
процесс
```

- Authorization выполняется до materialization.
- `request != grant` — обязательное правило.
- `luna-security` является центральным policy authority и работает в режиме fail closed.
- Криптографическая подпись, trust и authorization — разные решения.
- Для каждого `ApplicationInstance` mount namespace обязателен.
- PID namespace по умолчанию не создаётся.
- Приложение остаётся обычным non-1 процессом системного PID namespace.
- Landlock, credentials, capabilities и другие Linux primitives применяются по policy.
- `CAP_SYS_ADMIN` и эквивалентный host-level доступ не выдаются приложению по умолчанию.
- `cgroups v2` — принятый primitive для ограничения ресурсов.
- `luna-app-runtime` владеет lifecycle `ApplicationInstance` и не является вторым init или system supervisor.

## 14. UserSession и desktop

- `UserSession` — доменная сущность, а не daemon.
- `luna-system-runtime` владеет коллекцией сессий и координацией их lifecycle.
- Несколько UserSession могут существовать одновременно.
- Принятые состояния сессии: ACTIVE, RESTRICTED и TERMINATED.
- Выход из активной desktop session по умолчанию переводит её в поведение RESTRICTED.
- Граница входа является графической; authentication завершается до предоставления активной сессии.
- Recovery является отдельным исключением: виртуальный пользователь `recovery`, созданный из Recovery DATA provider, получает активный Recovery UserSession без интерактивного login provider.
- TTY не является обычным механизмом login.
- Wayland — принятое направление интеграции дисплея.
- Выбранная desktop-среда: `niri + Noctalia Shell`.
- Выбранная terminal-среда: `Ghostty + fish`.
- Desktop-компоненты находятся вне архитектуры core PID 1/system state.

## 15. Устройства и внешние тома

- `luna-device-manager` владеет discovery и lifecycle устройств и томов.
- Внешние тома должны автоматически появляться с понятной пользователю идентичностью, а не с raw `/dev/...` paths.
- Состояние управляемых томов хранится в `LUNA-DATA/system/volumes`.
- Приложения получают контролируемый доступ к устройствам и томам через security и namespace model.
- USB-носители не должны молча автоматически запускать приложения; policy может требовать явного подтверждения.
- Точный production mount backend остаётся задачей реализации.

## 16. Конфигурация и состояние

- Для человекочитаемой конфигурации и метаданных, где это уместно, предпочтителен TOML.
- Общесистемная изменяемая конфигурация находится в `LUNA-DATA/system/config`.
- Пользовательская конфигурация находится в `LUNA-DATA/users/<user>/config`.
- Приоритет конфигурации зависит от семантического класса ресурса; user overrides/defaults применяются только там, где это разрешено.
- `luna-state` владеет абстракцией durable state и моделью revision.
- Постоянное системное состояние хранится в `LUNA-DATA/system/state`.
- Начальный persistent backend — `redb`.
- Транзакции состояния атомарны и учитывают revision.
- Учёт boot attempts отделён от общего durable system state.

## 17. Менеджеры и владение runtime

- `luna-system-manager` владеет семантикой системных target/state.
- `luna-update-manager` владеет orchestration изменений и update transactions.
- `luna-kernel-manager` владеет inventory и selection kernel.
- `luna-device-manager` владеет жизненным циклом устройств и томов.
- `luna-app-manager` владеет установкой Bundle и lifecycle его данных.
- `luna-system-runtime` — единственный долгоживущий system-wide supervisor/runtime.
- `luna-app-runtime` владеет lifecycle выполнения приложений.
- `luna-user-session` содержит состояние доменной сущности UserSession и не является session daemon.
- Не существует generic `luna-runtime`, `luna-session`, `luna-run-session`, `luna-app-init` или `luna-core`.
- `RuntimeSpec` описывает выбранную для процесса libc: `musl` или `glibc`. `RuntimeProfile` описывает набор доверенных системных логических ресурсов и не является отдельным архитектурным слоем.

## 18. Обновления, checkpoints и откат

- Updates System Image и kernel версионируются независимо, но должны разрешаться в совместимый boot target.
- Update workflow следует принятому направлению prepare/checkpoint/apply/verify/commit.
- Прерванные транзакции сверяются с durable operation state.
- Rollback является явным и видимым пользователю; automatic rollback разрешён только там, где это требует принятая health/boot policy.
- Checkpoints отделены от logical state database.
- Btrfs snapshots — принятое направление реализации checkpoint/rollback там, где persistent storage backend их поддерживает.
- Удаление активного System Image требует подтверждения, что нужные runtime resources уже материализованы независимо от этого image.
- Delta updates являются transport/update functionality и не входят в RFC-0002 `.lbp`.

## 19. Формат Bundle

RFC-0002 Bundle Format v1 принят.

- `.lbp` — transport/archive формат Bundle.
- Фиксированный заголовок `LBP1` имеет 64 байта; записи секций — 64 байта.
- Целые числа кодируются little-endian.
- MANIFEST и PAYLOAD обязательны; RESOURCES и SIGNATURE опциональны.
- Manifest является canonical TOML.
- PAYLOAD — deterministic TAR с canonical metadata; zstd — canonical compression policy.
- ContentIdentity — BLAKE3-256 от canonical semantic content и не зависит от имени файла или расположения.
- v1 исключает symlinks, hard links и специальные filesystem entries.
- Ed25519 signatures опциональны и отделены от trust и authorization.
- Неизвестные major versions и malformed/unsafe containers отклоняются.
- Импорт `.deb` и `.rpm` относится к `luna-app-manager`, а не изменяет формат Bundle.

## 20. Runtime и асинхронность

- Для каждого процесса выбирается одна libc: `musl` или `glibc`.
- `musl` используется в native Luna userspace. `glibc` используется как compatibility environment для приложений, которым она необходима.
- Один процесс не смешивает две libc; разные ApplicationInstances могут использовать разные варианты libc.
- Tokio является принятым async runtime там, где действительно требуется асинхронное выполнение; не все компоненты обязаны использовать Tokio.
- Storage abstractions должны оставаться синхронными, если конкретный backend не требует иного.
- Generic runtime resolver/service не вводится.

## 21. Защита ресурсов и hardening

- Resource controls защищают system/runtime/diagnostic capacity от давления приложений.
- `cgroups v2` — принятый kernel primitive для контроля CPU/memory/process resources.
- Ограничения числа процессов и file descriptors входят в модель защиты.
- При memory pressure следует сначала освобождать disposable resources, а не system-critical resources.
- `fs-verity` — принятый primitive целостности там, где он поддерживается для immutable content.
- Интеграция IMA принята как future/production hardening, но не является начальной зависимостью.
- TPM measured boot — опциональное future hardening и не требуется для первоначальной реализации.

## 22. Сервисы и коммуникация

- Модель управления сервисами, близкая к OpenRC, является принятым направлением; точная реализация и интеграция остаются открытыми.
- GUI и CLI являются thin clients над общими backend contracts.
- Unix-socket IPC с небольшим versioned structured/binary protocol — принятое направление для межкомпонентного взаимодействия.
- Внутренние API версионируются; breaking changes требуют явного major API change.
- D-Bus, если используется, должен быть filtered/limited, а не предоставлять неограниченную шину host.

## 23. Дисциплина разработки

- Архитектура согласуется до реализации новых границ ответственности.
- Предпочтительный порядок: Architecture → RFC/format → interfaces → prototype → implementation → integration.
- Существующий код является свидетельством состояния реализации, но не источником архитектурной истины.
- Код Rust должен оставаться понятным и по возможности учебно доступным.
- Архитектурные изменения требуют явного одобрения пользователя до реализации.
- Компонент не должен молча расширять свою ответственность по принципу «лишь бы работало».
- Документация должна различать принятую архитектуру, статус реализации и исторические материалы.

## 24. Дополнительное поведение приложений и сессий

- Несколько пользователей могут одновременно иметь активные сессии.
- При выходе пользователя из активной desktop session поведение по умолчанию — RESTRICTED; политика конкретного пользователя/сессии может вместо этого продолжить или завершить приложения.
- Системные сервисы и update operations могут продолжаться между переключениями пользователей, если это безопасно.
- Доступ приложения к внешним томам, устройствам и пользовательским местам хранения контролируется разрешениями, а не предоставляется без ограничений.
- Концептуальные измерения filesystem access включают visibility/read/write; финальный security contract может уточнить их.
- `luna-app-manager` не должен молча загружать произвольные отсутствующие dependencies. Он должен определить требование, по возможности найти подходящий источник, объяснить операцию и получить требуемое authorization/confirmation.
- Application Bundles являются immutable installed units; намеренное изменение опытным пользователем находится вне обычного пути установки.
- Удаление Bundle не удаляет автоматически пользовательские данные; очистка выполняется по явному действию пользователя или политике хранения.
- Portable/external Bundles могут запускаться после inspection, integrity, trust и authorization checks.

## 25. Административные полномочия и защита данных

- Административные полномочия предоставляются явно и контролируются policy; приложения по умолчанию не получают постоянного host-level administrative privilege.
- Обязательного архитектурного слоя `sudo`/`su` нет.
- Механизм recovery key/password recovery остаётся принятым направлением на будущее.
- Шифрование DATA — опциональная future installation/user policy: может шифроваться весь `LUNA-DATA` и/или отдельные пользовательские данные.
- Шифрование не требуется для первоначальной реализации архитектуры.

## 26. Направления platform integration

- Wayland — принятое направление интеграции дисплея.
- PipeWire — принятое направление audio/media backend.
- Модель управления сервисами, близкая к OpenRC, принята как направление; точная интеграция остаётся открытой.
- Unix-socket IPC с небольшим versioned structured/binary protocol — принятое направление межкомпонентного взаимодействия.
- GUI и CLI являются thin clients над общими backend operations и contracts.
- При использовании D-Bus доступ должен быть filtered/limited, а не предоставлять unrestricted host bus.

## 27. Явно сохранённые открытые вопросы

Архив содержит вопросы, которые не были окончательно закрыты. Они остаются открытыми и не должны угадываться в SoT: точная грамматика манифеста System Image, точная грамматика kernel metadata, точный алгоритм разметки установщика, точная политика физических файловых систем, детальный UX/API разрешений приложений, точный device-mount backend, реализация service manager и полный production-механизм hybrid materialization.

Эти открытые вопросы не дают права вводить новый архитектурный компонент или слой без отдельного согласования.
