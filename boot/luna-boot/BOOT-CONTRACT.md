# Контракт `luna-boot`

**Статус:** принятые границы загрузчика; Luna Boot Handoff ABI v1 принят 2026-09-08.  
**Компонент:** UEFI-загрузчик Project Luna, вне обычного userspace workspace.

## 1. Граница firmware

`luna-boot.efi` начинается в UEFI Boot Services. Firmware передаёт image handle и system table. Загрузчик обязан определить собственное устройство и не должен выбирать произвольный `SimpleFileSystem` handle.

Производственный System partition использует ext4. `luna-boot` работает с системным устройством через UEFI Block I/O и read-only ext4 reader. UEFI Simple File System применяется для ESP самого загрузчика.

## 2. Boot key

Нет постоянной задержки для Boot Menu.

На входе `luna-boot` выполняет неблокирующее чтение доступного UEFI input buffer. Если `B`/`b` уже ожидает обработки, открывается Boot Menu. Иначе нормальная загрузка продолжается сразу.

## 3. Целевая пара

Boot target — это:

```text
System Image manifest
        +
compatible Linux kernel
```

`current` является обычной целью. Factory — сохранённой известной рабочей fallback-парой.

Manifest является источником image version и kernel compatibility metadata. Наличие kernel и image на диске само по себе не означает совместимость.

## 4. Kernel format

Для x86_64 используется стандартный Linux `arch/x86/boot/bzImage`.

`luna-boot` реализует Linux x86_64 boot protocol.

Загрузчик подготавливает `boot_params`, стандартные boot metadata, Linux command line при необходимости и `LunaBootHandoffV1`, затем получает финальный memory map, выполняет `ExitBootServices` и передаёт управление 64-bit kernel entry point.

## 5. Luna Boot Handoff

Luna-specific boot context передаётся как `LunaBootHandoffV1`, присоединённый к Linux x86 `setup_data`.

Handoff является memory-resident объектом и **никогда не является файлом** на SYSTEM, DATA или EFI.

Основные records v1:

```text
SYSTEM_PARTITION
DATA_PARTITION
SYSTEM_IMAGE
KERNEL_IDENTITY
BOOT_MODE
BOOT_STATE
```

SYSTEM и DATA идентифицируются GPT disk GUID + partition GUID, а не Linux device names.

Подробный ABI:

```text
docs/contracts/LUNA-BOOT-HANDOFF-ABI-V1.md
```

Linux x86 boot protocol определяет `setup_data` как расширяемый механизм передачи boot data. citeturn880881search1turn880881search2

## 6. System Image handoff

`luna-boot` не монтирует SquashFS и не создаёт initramfs userspace.

Он выбирает и проверяет System Image, фиксирует его identity/digest в Handoff и передаёт управление Luna kernel.

После kernel startup `luna-init` получает этот context и самостоятельно строит System Environment.

## 7. No-initramfs policy

Производственный путь Luna не использует отдельный initramfs userspace слой.

Чтобы `luna-init` мог быть непосредственным initial userspace:

```text
kernel → luna-init
```

вместо:

```text
kernel → initramfs → luna-init
```

в kernel должны быть встроены все drivers/filesystems/crypto/platform support, без которых конкретный поддерживаемый boot profile не может достигнуть SYSTEM и запустить `luna-init`.

Optional post-boot functionality может использовать loadable kernel modules.

## 8. Kernel modules

Kernel и его module set являются одной versioned artifact identity.

Каноническая структура:

```text
SYSTEM/kernels/
└── <kernel-id>/
    ├── bzImage
    ├── kernel.toml
    └── modules/
        └── lib/modules/<kernel-release>/...
```

`luna-boot` не загружает эти modules для раннего доступа к SYSTEM. Модули предназначены для post-boot kernel functionality.

## 9. Fallback

Failure policy зависит от класса ошибки.

Для отказа System Image после успешного запуска совместимого kernel Luna может использовать предыдущий совместимый System Image без полного перезапуска, когда это технически и безопасно возможно.

Для kernel-level failure, включая panic, может потребоваться reboot. После reboot загрузчик применяет Boot State и выбирает другую совместимую рабочую комбинацию.

После исчерпания usable вариантов применяется Factory. Если Factory также недоступна, выбирается Recovery.

Это не превращает `luna-boot` в полноценный recovery manager.

## 10. Post-ExitBootServices

После `ExitBootServices` запрещены обращения к UEFI Boot Services, их allocator, console APIs и UEFI filesystem protocols.

Все данные, необходимые для Linux handoff, должны быть полностью сформированы заранее.

## 11. Ответственность загрузчика

`luna-boot` владеет только:

- UEFI boundary;
- firmware-side storage/image discovery;
- image/kernel selection;
- compatibility verification;
- boot state selection/fallback;
- boot mode;
- Linux boot-protocol setup;
- Luna Handoff construction;
- `ExitBootServices`;
- kernel handoff.

Он не владеет:

- `luna-init` runtime supervision;
- UserSession;
- application lifecycle;
- Bundle management;
- graphical desktop;
- обычным service management;
- пользователями и их данными.

## 12. Связанные контракты

```text
docs/contracts/LUNA-BOOT-HANDOFF-ABI-V1.md
docs/contracts/SYSTEM-IMAGE-CONTRACT.md
docs/contracts/KERNEL-CONTRACT.md
docs/contracts/BOOT-STATE-CONTRACT.md
docs/contracts/FAILURE-RECOVERY-CONTRACT.md
docs/contracts/LUNA-INIT-CONTRACT.md
```
