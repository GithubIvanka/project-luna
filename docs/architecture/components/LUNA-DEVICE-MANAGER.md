# `luna-device-manager`

**Статус:** архитектурная граница/scaffold; реальное discovery и automount ещё в разработке.

## Назначение

Управляет обнаружением и жизненным циклом аппаратных устройств и томов, которые должны быть представлены системе и desktop.

## Владеет

- discovery устройств;
- device identity/state;
- volume lifecycle;
- hotplug/hot-unplug;
- safe mount/unmount/eject orchestration на системной границе;
- публикацией device/volume events.

## Не владеет

Низкоуровневым filesystem API, Bundle mapping, application authorization, desktop widgets или UEFI boot.

## Внешние носители

Целевой сценарий:

```text
USB inserted
 ↓
discovery
 ↓
filesystem detection
 ↓
volume mount
 ↓
event
 ↓
file manager
```

Ручной `mount` не должен быть обязательным пользовательским сценарием.

## Безопасность

Доступ приложения к volume не следует считать разрешённым только из факта его монтирования. Policy для конкретного приложения проходит через `luna-security`.

## Зависимости

`luna-fs`, `luna-event`, `luna-security` и Linux device/filesystem mechanisms.

## Открыто

Реальный discovery backend, automount/eject lifecycle, removable-media policy и полная интеграция с desktop.