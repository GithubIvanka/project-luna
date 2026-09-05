# `luna-namespace`

**Статус:** initial Linux materialization backend реализован; production security integration продолжается.

## Назначение

Материализует уже разрешённый execution namespace приложения через Linux kernel primitives.

## Владеет

- создание и настройкой private mount namespace;
- controlled bind mounts;
- подготовкой RAM-backed logical root;
- transactional cleanup materialized resources при ошибке;
- низкоуровневой materialization части Root Mapping;
- kernel-level filesystem enforcement через Landlock;
- безопасным FD-based подключением доверенных физических источников;
- PID-namespace boundary, когда она включена в execution profile.

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

## PID namespace

При включённой PID isolation приложение не становится PID 1.

```text
application PID namespace
├── PID 1 → Luna namespace supervisor/init
└── PID 2+ → application process tree
```

PID 1 используется для child reaping, signal/lifecycle boundary и удержания namespace supervisor semantics. Реальный application executable запускается как PID 2 или выше.

Это не security-through-obscurity механизм: приложение всё равно не должно полагаться на конкретные PID values для security. Требование PID 2+ существует как правильная Linux lifecycle/supervision модель и исключает назначение application процесса специальной роли PID 1.

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

Authorization policy, Bundle parsing, UserSession lifecycle, process supervision, UEFI или пользовательским UI.

## Linux mechanisms

В основе используются существующие kernel primitives: mount namespaces, PID namespaces where enabled, tmpfs, bind mounts, `openat2`, `open_tree`, `mount_setattr`, `move_mount`, chroot и Landlock. Дополнительные namespaces/cgroups/seccomp подключаются только через соответствующие contracts.

## Ошибки

Если любой обязательный mount/materialization шаг не выполнен, namespace не считается готовым. Уже созданные mounts откатываются transaction cleanup. Слой выше отвечает за удаление самого staging directory и служебного runtime state.

## Зависимости

`luna-root-mapping`, `luna-security`, `luna-fs` и Linux namespace APIs.

## Открыто

- PID namespace supervisor/child-spawn implementation;
- точная signal/reaping semantics PID 1 supervisor;
- lazy System Image hydration/materialization implementation;
- полноценный filtered `/dev`;
- production handling ошибок mount и восстановления после аварийного завершения процесса;
- privileged Linux integration tests для реального unshare/mount/chroot/Landlock/PID path.
