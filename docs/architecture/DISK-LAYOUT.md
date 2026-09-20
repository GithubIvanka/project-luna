# Структура диска

## Физические разделы

```text
Disk
├── EFI
├── LUNA-SYS
├── LUNA-DATA
└── SWAP
```

Метки `LUNA-SYS` и `LUNA-DATA` являются каноническими именами физических разделов.

## Связь EFI и LUNA-SYS

EFI и `LUNA-SYS` образуют единую boot-пару и должны находиться на одном физическом диске. `luna-boot.efi` проверяет это и загружает Luna только из `LUNA-SYS` этого диска.

## LUNA-SYS

`LUNA-SYS` управляется ОС и не предоставляется как обычное пользовательское хранилище.

```text
LUNA-SYS/
├── images/
│   ├── luna-X.Y.Z.squashfs
│   ├── luna-X.Y.Z.toml
│   └── ...
├── cores/
│   ├── luna-X.Y.Z.init
│   ├── luna-X.Y.Z.toml
│   └── ...
├── kernels/
│   └── <kernel-id>/
│       └── ...
├── config/
│   ├── boot-state.toml
│   ├── luna-data.toml
│   └── ...
└── recovery/
    ├── recovery.squashfs
    └── recovery.toml
```

Пути вроде `/images` и `/cores` в коде bootloader являются путями относительно корня подключённого `LUNA-SYS`, а не отдельными разделами. Используется именно `LUNA-SYS` того же физического диска, что и EFI.

`LUNA-SYS/recovery/` содержит единственный канонический Recovery DATA Image и его manifest: `recovery.squashfs` + `recovery.toml`. Recovery DATA является общей GUI/recovery provider layer для набора версионированных System Images; отдельного Recovery System Image нет.

## LUNA-DATA

```text
LUNA-DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── firmware/
│   ├── libs/
│   ├── config/
│   ├── resources/
│   │   ├── fonts/
│   │   ├── icons/
│   │   ├── themes/
│   │   ├── cursors/
│   │   ├── sounds/
│   │   ├── locales/
│   │   └── translations/
│   ├── state/
│   ├── volumes/
│   └── ...
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

`apps`, `drivers`, `firmware`, `libs`, `config` и `resources` являются каноническими областями `LUNA-DATA/system`. `resources/` использует ту же классификацию, что и System Image: `fonts/`, `icons/`, `themes/`, `cursors/`, `sounds/`, `locales/` и `translations/`. `drivers/` и `firmware/` относятся к разным классам и не объединяются. `state` и `volumes` остаются mutable-only областями DATA и отсутствуют в System Image. Новые каталоги верхнего уровня требуют явного архитектурного одобрения.

## Расположение и привязка LUNA-DATA

`LUNA-DATA` может находиться на том же физическом диске, что EFI/`LUNA-SYS`, либо на другом.

`LUNA-SYS/config/luna-data.toml` хранит GUID диска и GUID раздела целевой `LUNA-DATA` для быстрого подключения. Normal boot сначала использует эту стабильную привязку.

Если настроенный DATA недоступен, normal boot переходит в Recovery и не выбирает посторонний раздел молча. Recovery ищет и показывает кандидатов; при нескольких валидных кандидатах пользователь явно выбирает нужный. Выбранная GUID-пара может быть записана обратно в `luna-data.toml`.

## Владение

`LUNA-SYS` содержит неизменяемые и версионируемые артефакты загрузки и системные источники ОС, а также необходимую управляемую ОС конфигурацию и состояние. `LUNA-DATA` содержит долговременное изменяемое состояние. Пользовательский и application contract не должен требовать знания физических разделов.

## Выбор файловой системы

Для текущей x86_64 PC-сборки:
- EFI — FAT32;
- LUNA-SYS — ext4;
- LUNA-DATA — btrfs;
- SWAP — swap.

Это не меняет формат System Image: она остаётся непосредственно SquashFS.

## Граница безопасности

Обычные пользователи и приложения не получают запись в `LUNA-SYS`. Запись системных артефактов выполняет контролируемый update tooling. `LUNA-DATA` является persistent mutable boundary и разрешается явно при загрузке.
