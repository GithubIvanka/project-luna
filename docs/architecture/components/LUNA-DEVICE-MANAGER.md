# `luna-device-manager`

## Назначение

Владеет доменной моделью и lifecycle устройств и внешних томов.

## Владеет

- идентичностью и состоянием устройств/томов;
- обнаружением и lifecycle томов через явный backend;
- публикацией состояния устройств и томов в вышестоящие компоненты.

## Не владеет

Authorization приложения, создание mount namespace или выбор EFI/LUNA-SYS при загрузке.

## Поведение томов

Внешние тома должны получать понятную пользовательскую идентичность вместо отображения сырых `/dev/...` путей. Автоматическое подключение допустимо согласно policy; USB media не должны молча запускать приложения.

Для каждого Volume различаются:

- `VolumeId` — стабильная identity;
- `label` — пользовательская метка файловой системы/тома;
- `native_name` — native Luna presentation alias, например `data` для `LUNA-DATA`;
- текущий `mount state` — состояние доступа к файловой системе;
- текущий Windows drive alias — только compatibility mapping.

Volume не превращается в глобальный каталог `/mnt/<name>`. При необходимости
доступа `luna-namespace` создаёт namespace-local `MountInstance` для выбранного
`VolumeRef`. Один Volume может иметь несколько независимых mount instances в
разных application namespaces.

`Volumes` является виртуальной коллекцией storage objects для file manager и
других Luna компонентов. Entry этой коллекции не обязан иметь публичный Linux
pathname.

Состояние управляемых томов и пользовательские preferences относятся к
`LUNA-DATA/system/volumes`. Динамические Windows letters являются runtime state;
в persistent registry хранится только explicit preference.

## Ответственность

`luna-device-manager` владеет discovery, Volume identity, label/native-name
metadata, availability/hotplug lifecycle и Windows compatibility assignments.
Он не решает application authorization и не подменяет `luna-namespace` при
materialization mount instances.

## Статус

Базовые device/volume types и query boundary существуют. Dynamic letter registry
существует как prototype. Полный hotplug discovery, persistent registry, secure
mount/eject backend и production `Volumes` API ещё не завершены.