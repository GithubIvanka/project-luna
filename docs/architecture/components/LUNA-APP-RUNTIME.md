# `luna-app-runtime`

**Статус:** `ApplicationInstance`, `ApplicationPlan` и authorized-process launch boundary реализованы; production lifecycle integration продолжается.

## Назначение

Владеет выполнением и жизненным циклом запущенных приложений.

## Владеет

- identity и state `ApplicationInstance`;
- `ApplicationPlan` и executable identity для конкретного запуска;
- lifecycle процессов приложения;
- подготовкой execution environment;
- связью экземпляра с `UserSession`;
- выбором runtime по `RuntimeSpec`;
- границей между authorization и namespace/process enforcement.

`RuntimeKind` является свойством `RuntimeSpec`, а не самостоятельным компонентом. Принятые semantics включают Luna, Glibc и Bundle runtime.

## Поток запуска

```text
Bundle declaration
  ↓
ApplicationPlan
  ↓
MappingPlan
  ↓
luna-security
  ↓
AuthorizedApplicationPlan
  ↓
ApplicationLaunchContext
  ↓
luna-namespace
  ↓
process spawn / exec
  ↓
ApplicationInstance
```

План проходит валидацию до authorization. Authorization возвращает отдельный `AuthorizedApplicationPlan`; namespace materialization и process creation не выполняются во время policy evaluation.

## Security boundary

`ApplicationPlan` не является grant. Он содержит requests, mapping context и executable identity. Только `ApplicationPlan::authorize()` может создать `AuthorizedApplicationPlan`.

```text
request ≠ grant

ApplicationPlan
    ↓ validate
luna-security
    ↓ Allow
AuthorizedApplicationPlan
    ↓
ApplicationLaunchContext
    ↓
luna-namespace / process launch
```

`Deny`, policy errors и неподдержанные `Constrained` decisions являются fail-closed. Launcher не принимает обычный `ApplicationPlan`, только уже авторизованный тип.

## Executable boundary

Executable path является частью plan и должен:

1. быть абсолютным;
2. не содержать parent traversal;
3. быть представлен в `MappingTable`.

Это предотвращает замену разрешённого logical executable произвольным physical path на launch boundary.

Полная спецификация схемы executable declaration в Bundle остаётся отдельным design question; текущий plan получает уже выбранный executable от orchestration layer.

## Launch context boundary

`ApplicationLaunchContext` является типизированным execution context для одного запуска и содержит:

- process-local Linux mount namespace;
- immutable System Image base root;
- отдельный staging parent для runtime state.

До создания staging directory context проверяется. Оба filesystem roots должны быть абсолютными, а staging parent должен находиться вне System Image base-root tree. Runtime state не должен записываться в immutable lower layer.

## Namespace materialization

`luna-namespace` получает только authorized execution context. Физические пути DATA остаются внутренней реализацией. Приложение работает через logical root.

Создание process staging и logical root происходит только после успешной authorization. При ошибке spawn временный staging root удаляется.

## ApplicationInstance

`ApplicationInstance` представляет один конкретный launched execution и хранит:

- instance identity;
- application identity/version;
- session identity;
- runtime specification;
- lifecycle state;
- supervised process identity, если процесс создан.

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

`luna-system-runtime` остаётся system-wide supervisor. `luna-app-runtime` владеет application execution lifecycle. Generic `luna-runtime` daemon отсутствует.

## Не владеет

Bundle install/remove, созданием UserSession, system-wide supervision, authorization policy, raw filesystem primitives или UEFI boot.

## Тестовый контракт

План проверяется отдельно от Linux mount/exec tests:

- inactive session отклоняется;
- невалидный bundle отклоняется;
- runtime/mapping mismatch отклоняется до authorization;
- executable вне mapping отклоняется;
- foreign principal отклоняется;
- `Deny` не создаёт authorized plan;
- `Allow` создаёт typed `AuthorizedApplicationPlan`;
- authorization ordering сохраняется;
- отказ останавливает дальнейшую authorization pipeline;
- invalid launch context отклоняет запуск до создания staging directory;
- staging внутри System Image root отклоняется;
- launcher принимает только authorized plan type.

Linux integration дополнительно проверяет cleanup staging root и lifecycle процесса.

## Открыто

Migration legacy `launch_authorized` path на typed `ApplicationPlan` boundary; production integration с IPC, полноценным lifecycle reconciliation, resource limits/cgroups, restart policy, user confirmation IPC и полным kernel enforcement.
