# `luna-namespace`

**Статус:** initial Linux materialization backend реализован; production security integration продолжается.

## Назначение

Материализует уже разрешённый execution namespace приложения через Linux kernel primitives.

## Владеет

- создание и настройкой private mount namespace;
- controlled bind mounts;
- подготовкой RAM-backed logical root;
- cleanup materialized resources;
- низкоуровневой materialization части Root Mapping;
- kernel-level filesystem enforcement через Landlock;
- безопасным FD-based подключением доверенных физических источников.

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

## Безопасное подключение физических ресурсов

Для production mappings physical source должен принадлежать явно выбранному system-runtime trusted source root. Bundle не может сам объявить новый trust root.

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
openat2(target relative to host root)
        ↓
target O_PATH fd
        ↓
move_mount(... MOVE_MOUNT_F_EMPTY_PATH | MOVE_MOUNT_T_EMPTY_PATH)
```

Таким образом source и target не проходят отдельную pathname-проверку перед attach: kernel objects фиксируются FD до операции монтирования. Read-only применяется к detached mount object до attachment, без pathname-based remount.

Низкоуровневый `secure_bind_mount()` сохраняется только для совместимости с внутренними/legacy callers; production profile path обязан использовать explicit trusted source root.

## Filesystem permissions

Declared `Read`, `Write` и `Execute` permissions преобразуются в Landlock ruleset. Пустой access set не получает rule.

`luna-namespace` только исполняет уже авторизованные права; policy decision принадлежит `luna-security`.

## Не владеет

Authorization policy, Bundle parsing, UserSession lifecycle, process supervision, UEFI или пользовательским UI.

## Linux mechanisms

В основе используются существующие kernel primitives: mount namespaces, tmpfs, bind mounts, `openat2`, `open_tree`, `mount_setattr`, `move_mount`, chroot и Landlock. Дополнительные namespaces/cgroups/seccomp подключаются только через соответствующие contracts.

## Ошибки

Если любой обязательный mount/materialization шаг не выполнен, namespace не считается готовым. Частично созданное окружение должно быть очищено.

## Зависимости

`luna-root-mapping`, `luna-security`, `luna-fs` и Linux namespace APIs.

## Открыто

- fully transactional cleanup всех mount objects и промежуточного состояния при частично неуспешной materialization;
- полноценный filtered `/dev`;
- production handling ошибок mount и восстановления после аварийного завершения.
