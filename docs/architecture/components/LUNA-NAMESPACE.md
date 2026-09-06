# `luna-namespace`

**Статус:** initial Linux materialization backend реализован; production security integration продолжается.

## Назначение

Материализует уже разрешённый execution namespace приложения через Linux kernel primitives.

## Владеет

- созданием и настройкой private mount namespace;
- controlled bind mounts;
- подготовкой RAM-backed logical root;
- transactional cleanup materialized resources при ошибке;
- низкоуровневой materialization части Root Mapping;
- kernel-level filesystem enforcement через Landlock;
- безопасным FD-based подключением доверенных физических источников;
- policy-driven Linux isolation primitives, когда они явно включены в execution profile.

`luna-namespace` не владеет отдельным application init/supervisor process. `luna-app-runtime` запускает приложение непосредственно как `ApplicationInstance`, а `luna-system-runtime` остаётся единственным системным supervisor/PID 1.

## Обязательный порядок

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
luna-namespace
```

`luna-namespace` не должен обходить `luna-security` и сам выдавать приложению разрешения.

## Logical root

Production materialization не создаёт обычный persistent Linux root tree и не использует System Image целиком как OverlayFS lower layer.

Сначала создаётся пустой staging mountpoint, затем внутри private mount namespace на нём создаётся tmpfs. Это и есть backing store логического `/`. Persistent staging path содержит только mountpoint и служебную metadata, но не содержимое root filesystem.

System Image остаётся внутренним immutable source. В logical root попадают только ресурсы, которые явно разрешены RuntimeProfile и MappingTable.

Boot/runtime system materialization использует ту же принципиальную модель: boot-critical immutable system content предварительно materializes в RAM, дополнительные immutable resources могут быть hydrated lazily, а volatile paths (`/dev`, `/proc`, `/sys`, `/run`, `/tmp`) создаются runtime-механизмами. Ни один application process не получает физический SYSTEM path как свой `/`.

## Process/PID model

PID isolation не является обязательной частью текущей модели ApplicationInstance.

```text
system PID namespace
└── PID 1 → luna-system-runtime
    ├── system/runtime processes
    ├── UserSession processes
    └── ApplicationInstance → application process
```

`luna-app-runtime` — архитектурный runtime-компонент, а не дополнительный PID 1. Нет `luna-app-init` и нет отдельного namespace supervisor process между `luna-app-runtime` и приложением.

Приложение запускается непосредственно как обычный process, поэтому ему не назначается специальная роль PID 1 и оно получает обычный системный PID. Это также не является security-through-obscurity механизмом: изоляция обеспечивается mount namespace, policy-driven namespaces, cgroups и kernel security controls, а не сокрытием PID.

Если отдельный PID namespace когда-либо понадобится для конкретного требования, его семантика должна быть оформлена отдельным архитектурным решением и не должна автоматически создавать новый runtime layer.

## Безопасное подключение физических ресурсов

Для production mappings physical source должен принадлежать явно выбранному system-runtime trusted source root. Bundle не может сам объявить новый trust root.

Source и target имеют независимые границы доверия. Source разрешается только под explicit trusted source root, а target — только под explicit per-launch logical-root destination. Ни один production target не разрешается относительно host `/`.

Проверка и подключение выполняются через FD-based path resolution:

```text
trusted source root fd
        ↓
openat2(relative source)
RESOLVE_BENEATH
RESOLVE_NO_SYMLINKS
RESOLVE_NO_MAGICLINKS
        ↓
O_PATH fd
        ↓
open_tree(... AT_EMPTY_PATH | OPEN_TREE_CLONE)
        ↓
mount_setattr(... MOUNT_ATTR_RDONLY)   [если read-only]
        ↓
detached mount fd
        ↓
trusted logical-root fd
        ↓
openat2(target relative to logical root)
        ↓
target O_PATH fd
        ↓
move_mount(... *_EMPTY_PATH)
```

Таким образом source и target фиксируются через directory/file descriptors до attach. Read-only применяется к detached mount object до attachment, без pathname-based remount.

Низкоуровневый `secure_bind_mount()` сохраняется только для legacy/internal callers; production profile path обязан использовать explicit source и target roots.

## Transactional cleanup

Materialization регистрирует каждый успешно установленный mount в локальной transaction. При любой последующей ошибке mounts снимаются в обратном порядке через `umount2(..., MNT_DETACH)`. Transaction commit выполняется только после завершения всего logical-root materialization.

После успешного запуска cleanup staging root выполняется `luna-app-runtime` при exit/reconcile/terminate; сам mount namespace дополнительно уничтожается kernel при завершении дочернего процесса.

## Filesystem permissions

Declared `Read`, `Write` и `Execute` permissions преобразуются в Landlock ruleset. Пустой access set не получает rule.

`luna-namespace` только исполняет уже авторизованные права; policy decision принадлежит `luna-security`.

## Не владеет

Authorization policy, Bundle parsing, UserSession lifecycle, system process supervision, UEFI или пользовательским UI.

## Linux mechanisms

В основе используются существующие kernel primitives: mount namespaces, policy-driven Linux namespaces, tmpfs, bind mounts, `openat2`, `open_tree`, `mount_setattr`, `move_mount`, chroot и Landlock. `cgroups v2`, seccomp и дополнительные namespaces подключаются только через соответствующие contracts.

## Ошибки

Если любой обязательный mount/materialization шаг не выполнен, namespace не считается готовым. Уже созданные mounts откатываются transaction cleanup. Слой выше отвечает за удаление самого staging directory и служебного runtime state.

## Зависимости

`luna-root-mapping`, `luna-security`, `luna-fs` и Linux namespace APIs.

## Открыто

- lazy System Image hydration/materialization implementation;
- полноценный filtered `/dev`;
- production child-creation primitive для namespace setup, если текущий `pre_exec` path будет заменён;
- production handling ошибок mount и восстановления после аварийного завершения процесса;
- privileged Linux integration tests для реального unshare/mount/chroot/Landlock path.
