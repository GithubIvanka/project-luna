# `luna-app-runtime`

**Статус:** `ApplicationInstance`, явная execution state machine, `ApplicationPlan`, `RuntimeSpec`/`RuntimeKind` и typed authorized-process launch boundary реализованы; полноценный kernel/provider enforcement и durable recovery продолжаются.

## Назначение

Владеет выполнением и жизненным циклом запущенных приложений.

`luna-app-runtime` является application-runtime boundary между `UserSession` и конкретными `ApplicationInstance`. Внутри одного `ApplicationInstance` используются специализированные слои/subsystems: `ApplicationPlan`, `luna-root-mapping`, `luna-security` и внутренний `trusted-setup`.

## Иерархия

```text
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
    │
    ├── ApplicationPlan
    ├── luna-root-mapping
    │      └── MappingPlan
    ├── luna-security
    │      └── AuthorizedApplicationPlan
    └── trusted-setup
```

Это ownership/structural hierarchy, а не набор отдельных процессов. Все перечисленные слои работают в рамках lifecycle одного `ApplicationInstance`.

## Владеет

- identity и state `ApplicationInstance`;
- `ApplicationPlan` и executable identity для конкретного запуска;
- `RuntimeSpec` и выбор runtime semantics;
- lifecycle процессов приложения;
- подготовкой execution environment;
- связью экземпляра с `UserSession`;
- границей между authorization и namespace/process enforcement;
- внутренним trusted setup layer для материализации уже авторизованного execution environment и последующего `execve()`.

`RuntimeKind` является свойством `RuntimeSpec`, а не самостоятельным компонентом. Принятые semantics включают Luna, Glibc и Bundle runtime.

## ApplicationInstance

`ApplicationInstance` представляет один конкретный launched execution и хранит:

- instance identity;
- application identity/version;
- session identity;
- `RuntimeSpec`, включая `RuntimeKind`;
- lifecycle state;
- typed supervised process identity/PID, если процесс создан;
- terminal process outcome (`exit code`, `signal` или unknown abnormal outcome);
- typed failure stage и диагностическое сообщение для runtime/setup failures.

Process identity сохраняется после завершения процесса вместе с exit/crash outcome. Mutable lifecycle/process APIs не экспортируются: внешний caller может только наблюдать instance, а переходы выполняются внутри `luna-app-runtime`.

`ApplicationInstance` не принимает security decisions. Authorization, mapping validation и capability approval должны завершиться до запуска процесса.

## Lifecycle state machine

Новый instance создаётся в `Created`. Поддерживаемые переходы ограничены runtime-контрактом:

```text
Created  → Starting
Starting → Running
Starting → Failed
Running  → Stopping
Running  → Stopped
Running  → Crashed
Running  → Failed
Stopping → Stopped
Stopping → Failed
```

`Stopped`, `Crashed` и `Failed` являются terminal states и не имеют исходящих переходов. В частности, `Stopped → Running`, `Failed → Running` и `Created → Stopped` запрещены.

Нормальный самостоятельный exit процесса переводит instance в `Stopped`. Ненулевой exit или signal переводит его в `Crashed`. Явно запрошенное и успешно завершённое runtime termination проходит через `Stopping → Stopped`; ошибка termination переводит `Stopping → Failed`.

## RuntimeSpec / RuntimeKind

`RuntimeSpec` описывает execution runtime для конкретного `ApplicationInstance`. `RuntimeKind` является его классификацией и не существует как отдельный runtime-компонент.

```text
ApplicationInstance
    ↓
RuntimeSpec
    └── RuntimeKind
```

Runtime selection является частью `luna-app-runtime`. Runtime/mapping incompatibility должна быть выявлена до authorization.

## Поток запуска

```text
ApplicationInstance / Created
  │
  ├── RuntimeSpec / RuntimeKind
  │
  ├── ApplicationPlan
  │      ↓
  ├── validate
  │      ↓
  ├── luna-root-mapping
  │      ↓
  ├── MappingPlan
  │      ↓
  ├── luna-security
  │      ↓
  ├── AuthorizedApplicationPlan
  │      ↓
  └── trusted setup layer / Starting
           ↓
      Linux process + execution environment
           ↓
         execve()
           ↓
      ApplicationInstance / Running
```

Показанный pipeline описывает внутреннюю последовательность работы `ApplicationInstance`; `luna-root-mapping`, `luna-security` и `trusted-setup` не являются самостоятельными sibling runtime-компонентами уровня `UserSession`.

`luna-root-mapping` владеет семантикой logical-to-physical/resource mapping и строит `MappingPlan` из manifest/resource declarations и runtime/user/system context. `MappingPlan` не является security grant.

План проходит валидацию до authorization. Authorization возвращает отдельный `AuthorizedApplicationPlan`; namespace materialization и process creation не выполняются во время policy evaluation.

## Mapping boundary

`luna-app-runtime` не создаёт mapping policy напрямую и не является альтернативным владельцем `MappingPlan`.

```text
ApplicationPlan
    ↓
luna-root-mapping
    ↓
MappingPlan
    ↓
luna-security
    ↓
AuthorizedApplicationPlan
```

`luna-root-mapping` отвечает за logical path semantics, source selection, file/subtree mappings, dependency/resource mappings, validation и построение `MappingPlan`.

`luna-app-runtime` владеет lifecycle и только координирует использование этих слоёв в рамках `ApplicationInstance`.

## Security boundary

`ApplicationPlan` не является grant. Он содержит requests, mapping context и executable identity. Только authorization через `luna-security` может создать `AuthorizedApplicationPlan`.

```text
request ≠ grant

ApplicationPlan
    ↓ validate
luna-root-mapping
    ↓
MappingPlan
    ↓
luna-security
    ↓ Allow
AuthorizedApplicationPlan
    ↓
trusted setup layer
    ↓
luna-namespace / process launch primitives
```

`Deny`, policy errors и неподдержанные `Constrained` decisions являются fail-closed. Runtime и Linux launcher принимают только уже авторизованный тип; обычный `ApplicationPlan` не может пересечь launch boundary.

Capability identity также отделена от authorization: `CapabilityRegistry` определяет известный capability и provider, а `CapabilityGrant` появляется только после успешной authorization. Provider не принимает policy decision и не может расширить выданный grant.

## Session boundary

Authorized launch требует явный текущий `UserSession`. Непосредственно перед любыми staging/process side effects runtime повторно проверяет:

- session существует как typed launch argument;
- session находится в `Active`;
- session identity совпадает с identity, зафиксированной в `AuthorizedApplicationPlan`.

Inactive или foreign session приводит к fail-closed отказу без создания staging root и process. Это защищает от запуска уже авторизованного плана после logout/session replacement.

## Trusted setup layer

Trusted setup является **внутренним слоем `luna-app-runtime`**, а не отдельным постоянным daemon или самостоятельной security authority.

Он отвечает только за материализацию `AuthorizedApplicationPlan` в Linux execution environment и переход к приложению:

```text
AuthorizedApplicationPlan
        ↓
trusted setup layer
        ├── process creation
        ├── mount namespace
        ├── RAM-backed logical `/`
        ├── authorized resource mappings
        ├── runtime filesystems
        ├── final security restrictions
        └── execve()
```

Trusted setup **не может** добавлять mappings вне `AuthorizedApplicationPlan`, расширять security grants, разрешать denied resources, выдавать новые capability grants, расширять resource limits или обходить `luna-security`.

Trusted setup является фазой подготовки того же процесса, который после завершения подготовки делает `execve()`. Отдельный постоянный environment-helper daemon не является частью принятой архитектуры.

## RuntimeProfile и logical root

`RuntimeProfile` — явный набор trusted logical resources, которые система предоставляет execution environment независимо от пользовательских DATA mapping.

Текущий baseline-профиль `minimal` описывает `/lib`, `/lib64` и `/usr`; полный `/etc` не входит в профиль.

`luna-namespace` материализует профиль в отдельный RAM-backed logical root. Production launch path не использует полный System Image как OverlayFS lower и не создаёт persistent upper/work слой для `/`.

Физический System Image остаётся immutable source. Он не становится application `/` и не раскрывается приложению целиком. Приложение получает только те системные и собственные ресурсы, которые входят в его authorized execution context.

## Per-application Linux environment

Каждый запуск получает собственный Linux-shaped environment. Приложение видит привычную Linux иерархию, насколько её сформировал `RuntimeProfile` и namespace runtime, но не получает автоматического доступа ко всем физическим ресурсам этих путей.

```text
Application sees
    ↓
logical `/`
/usr /lib /lib64 /tmp /proc /sys /dev ...

Application may access
    ↓
only explicitly authorized resources

Application does NOT automatically access
    ↓
host filesystem
SYSTEM
other users
other applications
privileged devices
host namespaces/services
```

Видимость и доступ — разные свойства. Каждое внешнее filesystem mapping и capability должны пройти policy authorization.

## PID boundary

По умолчанию ApplicationInstance не получает отдельный PID namespace. Приложение запускается как обычный процесс в нормальном system PID namespace и получает обычный non-1 PID.

`luna-system-runtime` остаётся PID 1 нормального Luna userspace process namespace. `luna-app-runtime` не является дополнительным init-процессом, и `luna-app-init` не существует.

## Executable boundary

Executable path является частью plan и должен быть абсолютным, не содержать parent/current-directory traversal syntax, быть представлен в `MappingTable` и иметь `Execute` access в Bundle declaration.

Проверка navigation syntax выполняется по исходному pathname до возможной нормализации `Path`, чтобы `.` и `..` не исчезали из security check.

## Launch context boundary

`ApplicationLaunchContext` является типизированным execution context для одного запуска и содержит process-local Linux namespaces, immutable System Image source, отдельный staging parent для runtime state и явно доверенные source roots.

До создания staging directory context проверяется. Все roots должны быть абсолютными, без `.`/`..`; staging parent должен находиться вне System Image base-root tree; trusted source roots не могут указывать на host root или staging content. Runtime state не записывается в immutable System Image.

## Namespace materialization

`luna-namespace` получает только authorized execution context и mapping policy. Физические пути DATA/SYSTEM остаются внутренней реализацией. Приложение работает через logical root.

Logical root создаётся как tmpfs в private mount namespace; staging directory на persistent storage является только mountpoint и не является backing store для `/`.

Для физических source paths используется FD-based source resolution: `openat2()` с containment/no-symlink restrictions, затем detached mount через `open_tree()` и attach через `move_mount`.

Создание process staging и logical root происходит только после успешной authorization и session revalidation. Не committed staging root защищён cleanup guard и удаляется при setup/spawn/exec failure. После process exit runtime пытается удалить staging root и namespace support directory. Результат cleanup сохраняется отдельно от process exit/crash outcome, поэтому cleanup failure наблюдаем и не скрывает исходный результат процесса.

## Ownership model

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

`luna-system-runtime` остаётся system-wide supervisor. `luna-app-runtime` владеет application execution lifecycle и внутренним trusted setup layer. Generic `luna-runtime` daemon отсутствует.

## Не владеет

Bundle install/remove, созданием UserSession, system-wide supervision, authorization policy, семантикой logical mapping или отдельным постоянным environment-helper daemon.

## Тестовый контракт

План и lifecycle проверяются отдельно от privileged Linux mount/exec tests:

- `Created → Starting → Running → Stopping → Stopped` разрешён;
- `Starting → Failed`, `Running → Crashed` и `Stopping → Failed` разрешены;
- terminal states не могут перейти обратно в `Running`;
- lifecycle/process mutation APIs недоступны внешнему caller;
- inactive и foreign session отклоняются на launch boundary;
- runtime принимает только `AuthorizedApplicationPlan`;
- authorization denial не достигает launch boundary;
- process identity, exit/crash outcome и отдельный cleanup outcome сохраняются;
- normal exit переводит instance в `Stopped`;
- abnormal exit переводит instance в `Crashed`;
- uncommitted staging root удаляется;
- invalid launch context отклоняет запуск до создания staging directory;
- trusted setup не может материализовать resource вне authorized plan;
- capability names неизвестные Registry не могут получить grant;
- default application launch не требует PID namespace supervisor.

Privileged Linux integration запускает реальный динамический ELF через authorized launcher и проверяет отдельный mount namespace, fresh tmpfs `/`, trusted `/usr`/`/lib`/`/lib64`, authorized file/subtree mappings, kernel-enforced Landlock denial, FD policy и cleanup outcome.

## Открыто

Production-safe child creation protocol, который отдельно передаёт trusted-setup и final `execve()` diagnostics вместо зависимости от `Command::pre_exec`; durable lifecycle recovery после restart supervisor; target-side mount containment; trust-domain validation физических source paths; фактический capability IPC/provider invocation; resource limits/cgroups; restart policy; user confirmation IPC; lazy System Image hydration implementation; filtered `/dev`; `/proc` visibility model; `/sys` visibility model; полноценный kernel enforcement.


## P0 launch invariants

`luna-security` единолично создаёт sealed `AuthorizedApplicationPlan`. Production launcher использует только fresh tmpfs root, trusted runtime profile и authorized application mappings. Перед `execve()` все неразрешённые FD с номерами 3 и выше помечаются close-on-exec. Landlock ruleset объединяет trusted runtime resources и authorized application mappings, не раскрывая весь System Image.
