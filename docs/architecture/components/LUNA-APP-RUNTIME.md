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
- границей между authorization и namespace/process enforcement.

`RuntimeKind` является свойством `RuntimeSpec`, а не самостоятельным компонентом. Принятые semantics включают Luna, Glibc и Bundle runtime.

## Поток запуска

```text
Bundle declaration
  ↓
ApplicationPlan
  ↓
validate
  ↓
luna-security
  ↓
AuthorizedApplicationPlan
  ↓
ApplicationLaunchContext + RuntimeProfile
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

Capability identity также отделена от authorization: `CapabilityRegistry` определяет известный capability и provider, а `CapabilityGrant` появляется только после успешной authorization. Provider не принимает policy decision и не может расширить выданный grant.

## RuntimeProfile

`RuntimeProfile` — явный набор trusted logical resources, которые система предоставляет execution environment независимо от пользовательских DATA mapping.

Текущий baseline-профиль `minimal` описывает:

```text
/etc
/lib
/lib64
/usr
```

Профиль является контрактом уровня values. Фактическое bind/mount materialization принадлежит `luna-namespace`; capability/device exposure не становится implicit частью RuntimeProfile.

Важно: текущий namespace backend всё ещё использует полный System Image как OverlayFS lower layer. Это не считается финальной реализацией A3 и является отдельным hardening item: production path должен перейти на profile-driven system view, а не раскрывать весь SYSTEM.

## Executable boundary

Executable path является частью plan и должен:

1. быть абсолютным;
2. не содержать parent traversal;
3. быть представлен в `MappingTable`;
4. иметь `Execute` access в Bundle declaration.

Это предотвращает замену разрешённого logical executable произвольным physical path на launch boundary.

## Launch context boundary

`ApplicationLaunchContext` является типизированным execution context для одного запуска и содержит:

- process-local Linux mount namespace;
- immutable System Image base root;
- отдельный staging parent для runtime state.

До создания staging directory context проверяется. Оба filesystem roots должны быть абсолютными, а staging parent должен находиться вне System Image base-root tree. Runtime state не должен записываться в immutable lower layer.

## Namespace materialization

`luna-namespace` получает только authorized execution context и mapping policy. Физические пути DATA остаются внутренней реализацией. Приложение работает через logical root.

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
- launcher принимает только authorized plan type;
- capability names неизвестные Registry не могут получить grant.

Linux integration дополнительно проверяет cleanup staging root и lifecycle процесса.

## Открыто

Profile-driven System Image view вместо полного OverlayFS lower tree; фактический capability IPC/provider invocation; physical symlink/containment hardening; production lifecycle reconciliation; resource limits/cgroups; restart policy; user confirmation IPC; filtered `/dev`; полноценный kernel enforcement.
