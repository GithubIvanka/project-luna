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

Физический source path не должен проверяться через обычный `Path::is_file()`/`is_dir()` с последующим независимым `mount()`: между проверкой и операцией возможна pathname TOCTOU.

Текущий secure path использует:

```text
trusted source pathname
        ↓
openat2()
RESOLVE_BENEATH
RESOLVE_NO_SYMLINKS
RESOLVE_NO_MAGICLINKS
        ↓
O_PATH fd
        ↓
open_tree(... AT_EMPTY_PATH | OPEN_TREE_CLONE)
        ↓
detached mount fd
        ↓
move_mount(... MOVE_MOUNT_F_EMPTY_PATH)
```

Тем самым source object фиксируется kernel-level file descriptor до attach operation. Target path всё ещё должен иметь отдельный containment/trust-domain hardening.

## Filesystem permissions

Declared `Read`, `Write` и `Execute` permissions преобразуются в Landlock ruleset. Пустой access set не получает rule.

`luna-namespace` только исполняет уже авторизованные права; policy decision принадлежит `luna-security`.

## Не владеет

Authorization policy, Bundle parsing, UserSession lifecycle, process supervision, UEFI или пользовательским UI.

## Linux mechanisms

В основе используются существующие kernel primitives: mount namespaces, tmpfs, bind mounts, `openat2`, `open_tree`, `move_mount`, chroot и Landlock. Дополнительные namespaces/cgroups/seccomp подключаются только через соответствующие contracts.

## Ошибки

Если любой обязательный mount/materialization шаг не выполнен, namespace не считается готовым. Частично созданное окружение должно быть очищено.

## Зависимости

`luna-root-mapping`, `luna-security`, `luna-fs` и Linux namespace APIs.

## Открыто

Target-side containment для mount attachment, trust-domain validation физических source paths, полноценный filtered `/dev`, cleanup guarantees для всех mount objects и production handling ошибок mount.