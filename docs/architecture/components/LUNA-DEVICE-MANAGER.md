# `luna-device-manager`

## Назначение

Управляет обнаружением физических устройств и публикацией аппаратных событий.
Логические тома и их пользовательская доступность принадлежат отдельному
`luna-volume-manager`.

## Владеет

- обработкой kernel uevents и обнаружением оборудования;
- обнаружением блочных устройств и разделов через sysfs;
- сбором аппаратных метаданных файловой системы из доступной udev database;
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
приложений. Device Manager отправляет аппаратные события при подключении и
отключении; Volume Manager самостоятельно reconciles актуальный список
разделов и управляет логическим lifecycle.

## Hotplug

Daemon открывает kernel uevent monitor, запускает доступный udev-совместимый
backend для заполнения metadata, инициирует coldplug и затем продолжает
слушать события. Удаление устройства сообщает о физическом изменении, но
согласованное снятие namespace-local mounts и ревокация доступа относятся к
Volume Manager и namespace lifecycle.

## Статус

Реализованы sysfs block discovery, stable identity derivation для GPT
partition, best-effort filesystem metadata из udev database, input discovery и
native kernel uevent monitor. Политика auto-attach, volume registry, aliases и
Volume backend resolution вынесены в отдельный crate `luna-volume-manager`.
