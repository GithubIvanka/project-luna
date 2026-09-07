# `luna-app-runtime`

**Статус:** `ApplicationInstance`, `ApplicationPlan` и typed authorized-process launch boundary реализованы; production lifecycle integration и полноценный kernel/provider enforcement продолжаются.

## Назначение

Владеет выполнением и жизненным циклом запущенных приложений.

## Владеет

- identity и state `ApplicationInstance`;
- `ApplicationPlan` и executable identity для конкретного запуска;
- lifecycle процессов приложения;
- подготовкой execution environment;
- связью экземпляра с `UserSession`;
- выбором runtime по `RuntimeSpec`;
- границей между authorization и namespace/process enforcement;
- внутренним trusted setup layer для материализации уже авторизованного execution environment и последующего `execve()`.

`RuntimeKind` является свойством `RuntimeSpec`, а не самостоятельным компонентом. Принятые semantics включают Luna, Glibc и Bundle runtime.

## Поток запуска

```text
Bundle declaration
  ↓
ApplicationPlan
  ↓
validate
  ↓
luna-root-mapping
  ↓
MappingPlan
  ↓
luna-security
  ↓
AuthorizedApplicationPlan
  ↓
luna-app-runtime / trusted setup layer
  ↓
Linux process + execution environment
  ↓
execve()
  ↓
ApplicationInstance / Running
```

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

`luna-root-mapping` отвечает за:

- logical path semantics;
- source selection;
- file/subtree mappings;
- dependency/resource mappings;
- validation и построение `MappingPlan`.

`luna-app-runtime` только потребляет уже сформированный и авторизованный план в рамках launch lifecycle.

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

`Deny`, policy errors и неподдержанные `Constrained` decisions являются fail-closed. Launcher не принимает обычный `ApplicationPlan`, только уже авторизованный тип.

Capability identity также отделена от authorization: `CapabilityRegistry` определяет известный capability и provider, а `CapabilityGrant` появляется только после успешной authorization. Provider не принимает policy decision и не может расширить выданный grant.

## Trusted setup layer

Trusted setup является **внутренним слоем `luna-app-runtime`**, а не отдельным постоянным daemon или самостоятельной security authority.

Он отвечает только за материализацию `AuthorizedApplicationPlan` в Linux execution environment и переход к приложению:

```text
AuthorizedApplicationPlan
        ↓
trusted setup layer
        ├── process creation
        ├── cgroup placement
        ├── mount namespace
        ├── RAM-backed logical `/`
        ├── authorized resource mappings
        ├── runtime filesystems
        ├── final credentials/capabilities setup
        ├── final security restrictions
        └── execve()
```

Trusted setup **не может**:

- добавлять новые mappings вне `AuthorizedApplicationPlan`;
- расширять security grants;
- самостоятельно разрешать denied resources;
- выдавать capability grants, которых нет в authorized plan;
- расширять resource limits;
- обходить `luna-security`.

Таким образом:

```text
luna-root-mapping
    = что и откуда должно попасть в logical environment

luna-security
    = что действительно разрешено

trusted setup
    = как разрешённое материализуется средствами Linux
```

Trusted setup является фазой подготовки того же процесса, который после завершения подготовки делает `execve()`. Отдельный постоянный environment-helper daemon не является частью принятой архитектуры.

## RuntimeProfile и logical root

`RuntimeProfile` — явный набор trusted logical resources, которые система предоставляет execution environment независимо от пользовательских DATA mapping.

Текущий baseline-профиль `minimal` описывает:

```text
/etc
/lib
/lib64
/usr
```

`luna-namespace` материализует профиль в отдельный RAM-backed logical root. Production launch path не использует полный System Image как OverlayFS lower и не создаёт persistent upper/work слой для `/`.

Физический System Image остаётся immutable source. Он не становится application `/` и не раскрывается приложению целиком. Boot/runtime слой материализует boot-critical system base в RAM, а дополнительные immutable resources могут гидратироваться лениво. Приложение получает только те системные и собственные ресурсы, которые входят в его authorized execution context.

## Per-application Linux environment

Каждый запуск получает собственный Linux-shaped environment. Приложение видит привычную Linux иерархию, насколько её сформировал `RuntimeProfile` и namespace runtime, но не получает автоматического доступа ко всем физическим ресурсам этих путей.

```text
Application sees
    ↓
logical `/`
/etc /usr /lib /tmp /proc /sys /dev ...

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

Видимость и доступ — разные свойства. Наличие `/etc` не означает доступ ко всему физическому `/etc` хоста; наличие `/dev` не означает доступ к устройствам. Каждое внешнее filesystem mapping и capability должны пройти policy authorization.

Capabilities также не являются скрытым продолжением filesystem. Например, grant `network` означает только ту сетевую возможность, которую предоставляет runtime/provider; он не открывает host filesystem или произвольные namespaces.

## PID boundary

По умолчанию ApplicationInstance не получает отдельный PID namespace. Приложение запускается как обычный процесс в нормальном system PID namespace и получает обычный non-1 PID.

`luna-system-runtime` остаётся PID 1 нормального Luna userspace process namespace. `luna-app-runtime` не является дополнительным init-процессом, и `luna-app-init` не существует.

Если отдельный PID namespace когда-либо потребуется для конкретного сценария, это требует отдельного архитектурного решения и не должно молча добавлять новый runtime layer.

## Executable boundary

Executable path является частью plan и должен:

1. быть абсолютным;
2. не содержать parent/current-directory traversal syntax;
3. быть представлен в `MappingTable`;
4. иметь `Execute` access в Bundle declaration.

Проверка navigation syntax выполняется по исходному pathname до возможной нормализации `Path`, чтобы `.` и `..` не исчезали из security check.

## Launch context boundary

`ApplicationLaunchContext` является типизированным execution context для одного запуска и содержит:

- process-local Linux namespaces;
- immutable System Image source;
- отдельный staging parent для runtime state.

До создания staging directory context проверяется. Оба filesystem roots должны быть абсолютными, без `.`/`..`, а staging parent должен находиться вне System Image base-root tree. Runtime state не должен записываться в immutable System Image.

## Namespace materialization

`luna-namespace` получает только authorized execution context и mapping policy. Физические пути DATA/SYSTEM остаются внутренней реализацией. Приложение работает через logical root.

Logical root создаётся как tmpfs в private mount namespace; staging directory на persistent storage является только mountpoint и не является backing store для `/`.

Для физических source paths используется FD-based source resolution: `openat2()` с containment/no-symlink restrictions, затем detached mount через `open_tree()` и attach через `move_mount`. Это устраняет pathname TOCTOU между проверкой source и bind operation.

Создание process staging и logical root происходит только после успешной authorization. При ошибке spawn временный staging root удаляется.

## ApplicationInstance

`ApplicationInstance` представляет один конкретный launched execution и хранит:

- instance identity;
- application identity/version;
- session identity;
- runtime specification;
- lifecycle state;
- supervised process identity, если процесс создан.

`ApplicationInstance` не принимает security decisions. Authorization, mapping validation и capability approval должны завершиться до запуска процесса.

Состояние `Running` выставляется только после успешного создания и attach supervised process для production launcher.

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

План проверяется отдельно от Linux mount/exec tests:

- inactive session отклоняется;
- невалидный bundle отклоняется;
- runtime/mapping mismatch отклоняется до authorization;
- executable вне mapping отклоняется;
- navigation syntax `.`/`..` отклоняется;
- foreign principal отклоняется;
- `Deny` не создаёт authorized plan;
- `Allow` создаёт typed `AuthorizedApplicationPlan`;
- authorization ordering сохраняется;
- отказ останавливает дальнейшую authorization pipeline;
- invalid launch context отклоняет запуск до создания staging directory;
- staging внутри System Image root отклоняется;
- launcher принимает только authorized plan type;
- trusted setup не может материализовать resource вне authorized plan;
- trusted setup не может расширить capability/resource grant;
- capability names неизвестные Registry не могут получить grant;
- default application launch не требует PID namespace supervisor.

Linux integration дополнительно проверяет cleanup staging root, mount namespace lifecycle, process lifecycle и process reaping.

## Открыто

Target-side mount containment; trust-domain validation физических source paths; фактический capability IPC/provider invocation; production lifecycle reconciliation; resource limits/cgroups; restart policy; user confirmation IPC; lazy System Image hydration implementation; filtered `/dev`; `/proc` visibility model; `/sys` visibility model; полноценный kernel enforcement.
