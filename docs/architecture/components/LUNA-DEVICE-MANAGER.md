# `luna-device-manager`

## Назначение

Управляет обнаружением физических устройств и публикацией аппаратных событий.
Логические тома и их пользовательская доступность принадлежат отдельному
`luna-volume-manager`.

## Владеет

- обработкой kernel uevents и обнаружением оборудования;
- обнаружением блочных устройств и разделов через sysfs;
- сбором аппаратных метаданных файловой системы из доступной udev database;
- минимальным read-only probing сигнатур ext4/Btrfs, когда udev metadata отсутствует;
- идентификацией физического backend и определением признака removable;
- обнаружением input devices и их обновлением при hotplug.

## Не владеет

- реестром логических Volume и их availability;
- назначением и освобождением `C:` / `D:`–`Z:`;
- пользовательскими метками, native names и preferences томов;
- политикой auto-attach и безопасного извлечения;
- application authorization или mount namespaces.

## Граница обнаружения томов

`BlockVolumeDiscovery` возвращает `DiscoveredBlockVolume`: VolumeId-кандидат,
метаданные, роль, removable hint и backend node. Эти данные описывают
обнаруженную физическую файловую область, но не означают, что Luna уже сделала
том доступным приложению.

```text
kernel uevent / sysfs / udev metadata
                 ↓
         luna-device-manager
                 ↓
       DiscoveredBlockVolume
                 ↓
         luna-volume-manager
```

Сырые `/dev/...` пути остаются backend-деталями и не являются API для
приложений. При старте и изменении block inventory Device Manager публикует
атомарный snapshot в `/run/luna/device-manager.volumes`; это внутренний
временный контракт двух core-менеджеров. Volume Manager не сканирует sysfs и
не открывает kernel uevent monitor — он reconciles этот snapshot и управляет
логическим lifecycle.

## Hotplug

Daemon открывает kernel uevent monitor, запускает доступный udev-совместимый
backend для заполнения metadata, инициирует coldplug и затем продолжает
слушать события. Если udev отсутствует, native discovery определяет ext4 и
Btrfs по суперблочным сигнатурам без монтирования и записи; это минимальное
обнаружение покрывает LUNA-SYS/LUNA-DATA, но не заменяет полную filesystem
compatibility metadata для всех внешних форматов. Удаление устройства сообщает о физическом изменении, но
согласованное снятие namespace-local mounts и ревокация доступа относятся к
Volume Manager и namespace lifecycle.

## Статус

Реализованы sysfs block discovery, stable identity derivation для GPT
partition, best-effort filesystem metadata из udev database, read-only native
probe ext4/Btrfs, input discovery и native kernel uevent monitor. Политика
auto-attach, volume registry, aliases и
Volume backend resolution вынесены в отдельный crate `luna-volume-manager`.
