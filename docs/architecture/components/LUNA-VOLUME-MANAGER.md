# `luna-volume-manager`

## Назначение

Владеет логическими томами Luna: их identity, доступностью, пользовательскими
метаданными и aliases. Разделяет обнаружение физического оборудования и
использование тома конкретным приложением.

## Границы ответственности

### Владеет

- доменным реестром `VolumeInfo`, `VolumeId` и `VolumeRef`;
- состояниями `Detected`, `Available`, `Mounted`, `Unavailable`, `Removing`;
- policy автоматической доступности внутренних пользовательских томов и USB;
- метками, native names и пользовательскими preferences;
- dynamic `C:` / `D:`–`Z:` assignments;
- разрешением `VolumeRef` в backend details для доверенного namespace layer;
- reconcile add/update/remove и событиями `Attached`, `Updated`, `Detached`.

### Не владеет

- физическим обнаружением устройств и kernel uevents;
- application authorization;
- созданием Linux mount namespace и app-specific `MountInstance`;
- постоянным глобальным mount tree;
- остановкой приложений и полноценным safe-eject coordinator/IPC.

## Поток подключение → доступность

```text
kernel uevent / sysfs
        ↓
luna-device-manager
        ↓ DiscoveredBlockVolume
luna-volume-manager coldplug + hotplug reconcile
        ↓
VolumeRef becomes Available
        ↓
Volume Namespace / authorized app request
        ↓
namespace-local MountInstance
        ↓
file open
```

На старте Volume Manager дожидается coldplug readiness Device Manager, читает
опубликованный им снимок разделов и загружает durable preferences. Затем он
отслеживает атомарно обновляемый снимок: при появлении или отключении USB-носителя
Device Manager обрабатывает kernel uevent и публикует новый inventory, а Volume
Manager выполняет reconcile логического registry. Он сам не читает sysfs и не
подписывается на kernel uevents. Постоянный пользовательский mount или каталог
`/mnt/<disk>` для этого не создаётся.

## Auto-attach policy

Политика по умолчанию включена. Том с распознанной поддерживаемой файловой
системой и ролью `External` автоматически переходит в `Available`, независимо
от того, является носитель USB/removable или внутренним диском. `LUNA-DATA`
имеет зарезервированную identity и alias `C:`. Роли EFI, LUNA-SYS, SWAP и
RecoveryVirtual не становятся пользовательскими дисками.

Поддерживаемые текущим prototype файловые системы: btrfs, ext2/3/4, exFAT,
F2FS, NTFS/NTFS3, UDF, VFAT и XFS. Неизвестная файловая система остаётся в
состоянии `Detected` и не получает динамическую букву. Фактическое чтение
проверяется backend-подключением; наличие поддержки в policy не гарантирует,
что конкретное ядро собрано со всеми нужными драйверами.

**Auto-attach означает логическую доступность**, а не выполнение глобального
`mount`. При открытии тома доверенный caller строит разрешённый
`VolumePath`/MappingPlan, а `luna-namespace` создаёт отдельный mount instance
в namespace приложения. Один Volume может использоваться одновременно
несколькими приложениями без общей видимой mountpoint.

## Identity, aliases и preferences

- GPT partition GUID — предпочтительная стабильная identity.
- Filesystem UUID или block node используются discovery только как fallback.
- Label и native name являются presentation metadata, не identity.
- Буквы `D:`–`Z:` выдаются в момент attach и освобождаются при detach.
- `C:` зарезервирован для `LUNA-DATA`.
- Durable пользовательские preferences сохраняются в
  `LUNA-DATA/system/volumes/volume-registry.toml`; текущие назначения букв —
  runtime state.

## Состояние реализации

Реализованы crate и executable `luna-volume-manager`, policy auto-attach,
startup coldplug wait, reconciliation по атомарному inventory snapshot, persistent
preference backend, drive aliases и backend resolution. System Image build scripts
пакуют daemon, OpenRC запускает Device Manager и Volume Manager при загрузке,
а тесты проверяют доступность поддерживаемого тома и отказ для неизвестных
файловых систем/системных разделов.

Production IPC/API для списка Volume объектов и уведомлений, интеграция открытия
VolumePath с Luna Files, глобальное отслеживание всех `MountInstance` для
безопасного извлечения и privileged hardware smoke tests ещё не завершены.
