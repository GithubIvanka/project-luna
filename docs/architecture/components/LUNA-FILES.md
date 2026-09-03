# `luna-files`

**Статус:** GTK4 GUI присутствует; filesystem/volume integration ещё неполная.

## Назначение

Пользовательский file-manager client Luna. Он предоставляет удобный интерфейс к logical user files и подключённым volumes.

## Владеет

- навигацией по доступному пользователю logical filesystem;
- отображением файлов и каталогов;
- стандартными file operations;
- отображением volumes;
- presentation ошибок и permissions.

## Не владеет

Raw device discovery, mount policy, application authorization, Bundle lifecycle или kernel/storage backend.

## Внешние носители

File manager получает volume state от Luna device/volume boundary и не должен самостоятельно управлять `/dev` или изобретать mount policy.

## Application file access

Наличие file manager не является реализацией application portal. Доступ приложения к конкретному файлу должен быть отдельным security/portal contract.

## Provider

Yazi может поставляться как пользовательский инструмент, но факт его упаковки не доказывает прямую интеграцию `yazi-core` в Luna Files.

## Открыто

Полные file operations, navigation, volume integration, permissions/error UX и дальнейшая backend integration.