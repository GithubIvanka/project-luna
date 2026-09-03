# Project Luna — модель диска и хранения данных

**Статус:** принятая архитектура; часть implementation details ещё открыта.  
**Источник истины:** `docs/ARCHITECTURE.md` и принятые решения.

## 1. Физическая модель

```text
Disk
├── EFI
├── SYSTEM
├── DATA
└── SWAP
```

`SWAP` необязателен и может быть разделом, файлом или ZRAM. EFI и SYSTEM управляются ОС. DATA является основной изменяемой и пользовательской областью.

EFI/SYSTEM и DATA/SWAP допускается размещать на разных физических дисках.

## 2. EFI

Только UEFI boot infrastructure:

```text
EFI/
└── Luna/
    └── luna-boot.efi
```

Пользователь не должен просматривать или редактировать EFI в штатной работе.

## 3. SYSTEM

Назначение: immutable/versioned OS payload и kernels.

```text
SYSTEM/
├── images/
│   ├── luna-X.Y.Z.squashfs
│   ├── luna-X.Y.Z.toml
│   └── ...
└── kernels/
    └── ...
```

Точная структура metadata внутри `kernels/` закрепляется Kernel Contract.

SYSTEM не является обычной пользовательской writable root filesystem.

## 4. DATA

```text
DATA/
├── system/
│   ├── apps/
│   ├── drivers/
│   ├── libs/
│   ├── volumes/
│   ├── config/
│   └── state/
├── users/
│   └── <user>/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

`DATA/system/apps/` — общие установленные immutable Bundles.  
`DATA/system/drivers/` — OS-managed driver entities.  
`DATA/system/libs/` — адресуемые shared dependencies.  
`DATA/system/volumes/` — внутреннее состояние внешних томов.  
`DATA/system/config/` — машинная конфигурация.  
`DATA/system/state/` — durable state через `luna-state`.  
`DATA/users/<user>/` — пользовательское содержимое.  
`DATA/cache/` — disposable cache.

## 5. Logical `/`

Физическая структура разделов не должна напрямую становиться root приложений.

```text
physical storage
      ↓
controlled mapping
      ↓
logical Linux /
      ↓
per-application namespace
```

Bundle declarations не должны раскрывать физические `DATA/system/...` пути.

## 6. Filesystem implementation

Текущий PC image builder использует ext4 для writable SYSTEM/DATA filesystem images, при этом сами System Images остаются SquashFS. Это implementation choice текущего bring-up и не меняет формат System Image.

Btrfs может использоваться для checkpoint/rollback там, где это прямо предусмотрено соответствующим contract, но это не делает Btrfs обязательной файловой системой обычного DATA layout.

## 7. Изменяемость

```text
immutable/versioned:
    SYSTEM images
    boot-critical payload

versioned/independent:
    kernels

mutable:
    DATA
```

Пользовательские данные не зависят от версии System Image.

## 8. Инварианты

- System Images и kernels не хранятся в DATA.
- DATA не превращается в традиционную Linux root hierarchy.
- System Image immutable при обычной работе.
- пользовательское и application state находится вне image.
- EFI и SYSTEM являются OS-managed.
- физические пути не являются частью Bundle mapping semantics.
