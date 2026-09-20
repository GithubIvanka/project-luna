# `luna-boot.efi`

## Назначение

UEFI bootloader и authority выбора boot target. Компонент находится вне обычного Cargo workspace userspace.

## Владеет

- проверкой связи EFI и `LUNA-SYS` на одном физическом диске;
- discovery `LUNA-SYS` и доступного `LUNA-DATA`;
- чтением boot state;
- проверкой manifest;
- разрешением `System Image → luna-init → kernel`;
- исключительным Boot Menu по `B`;
- загрузкой kernel и memory-resident `luna-init`;
- подготовкой `LunaBootHandoffV1`;
- единственной записью `LunaBootAttempt` перед `ExitBootServices`;
- вызовом `ExitBootServices`.

## Не владеет

`luna-boot.efi` не управляет UserSession, приложениями, Bundle installation или durable target mutations, принадлежащими `luna-update-manager`.

## Жизненный цикл

```text
UEFI
  ↓
инициализация Boot Services
  ↓
discovery
  ↓
разрешение target
  ↓
Boot Menu при удержании B
  ↓
подготовка kernel + init + handoff
  ↓
LunaBootAttempt = in_progress
  ↓
ExitBootServices
  ↓
Linux kernel
```

## Совместимость

```text
System Image manifest
    ↓
совместимые luna-init
    ↓
manifest выбранного luna-init
    ↓
совместимые kernels
```

Несовместимые варианты исключаются ещё до загрузки.

## DATA

Сначала используется GUID-пара из `LUNA-SYS/config/luna-data.toml`. Если normal DATA не разрешается однозначно, boot policy переводит запуск в Recovery.

## Ошибки

Если не существует допустимого target, загрузка прекращается или переходит в Recovery согласно boot policy. Soft failure System Image/early userspace может быть обработан без reboot при работоспособном kernel. Kernel failure обрабатывается следующим запуском после reboot.

## Статус

Discovery, GPT/ext4 access, target selection, menu, kernel loading, external boot, memory preparation и handoff реализованы в репозитории. Полная production hardening ещё продолжается.