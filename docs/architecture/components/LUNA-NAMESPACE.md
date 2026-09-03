# `luna-namespace`

**Статус:** initial Linux materialization backend реализован; production security integration продолжается.

## Назначение

Материализует уже разрешённый execution namespace приложения через Linux kernel primitives.

## Владеет

- создание и настройкой mount namespace;
- controlled bind mounts;
- подготовкой namespace filesystem view;
- cleanup materialized resources;
- низкоуровневой materialization части Root Mapping.

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
luna-namespace
```

`luna-namespace` не должен обходить `luna-security` и сам выдавать приложению разрешения.

## Не владеет

Authorization policy, Bundle parsing, UserSession lifecycle, process supervision, UEFI или пользовательским UI.

## Linux mechanisms

В основе используются существующие kernel primitives, прежде всего mount namespaces и bind mounts. Дополнительные namespaces/cgroups/seccomp подключаются только через соответствующие contracts.

## Ошибки

Если любой обязательный mount/materialization шаг не выполнен, namespace не считается готовым. Частично созданное окружение должно быть очищено.

## Зависимости

`luna-root-mapping`, `luna-security`, `luna-fs` и Linux namespace APIs.

## Открыто

Полная integration security enforcement, resource isolation, cleanup guarantees и production handling ошибок mount.