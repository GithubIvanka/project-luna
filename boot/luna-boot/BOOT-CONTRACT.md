# Контракт `luna-boot`

**Статус:** принятые границы загрузчика; детали Phase 0 уточняются контрактами.  
**Компонент:** UEFI-загрузчик Project Luna, вне обычного userspace workspace.

## 1. Граница firmware

`luna-boot.efi` начинается в UEFI Boot Services. Firmware передаёт image handle и system table. Загрузчик обязан определить собственное устройство и не должен выбирать произвольный `SimpleFileSystem` handle.

Производственный System partition использует ext4. Поэтому `luna-boot` работает с системным устройством через UEFI Block I/O и использует read-only ext4 reader. UEFI Simple File System применяется только там, где firmware предоставляет ESP filesystem самого загрузчика.

## 2. Boot key

Нет двухсекундной задержки.

На входе `luna-boot` выполняет неблокирующее чтение доступного UEFI console input buffer. Если `B`/`b` уже ожидает обработки, открывается Boot Menu. Иначе нормальная загрузка продолжается сразу.

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

`luna-boot` реализует Linux x86_64 boot protocol и не обязан запускать EFI stub.

Загрузчик подготавливает `boot_params`, command line, initrd и memory information, выходит из UEFI Boot Services и передаёт управление 64-bit kernel entry point.

## 5. System Image handoff

`luna-boot` не монтирует SquashFS.

Выбранный System Image передаётся через boot context/kernel command line и initramfs, содержащий `luna-init`. Затем `luna-init` находит SYSTEM, открывает выбранный `.squashfs`, строит logical root и подключает DATA.

## 6. Fallback

Failure policy зависит от класса ошибки.

Для отказа System Image после успешного запуска совместимого kernel следует, если это технически и безопасно возможно, попробовать предыдущий совместимый System Image без полного перезапуска.

Для kernel-level failure, включая panic, может потребоваться reboot. После reboot загрузчик применяет Boot State и выбирает другую совместимую рабочую комбинацию.

После исчерпания usable вариантов применяется Factory. Если Factory также недоступна, выбирается Recovery.

Это не означает, что `luna-boot` становится полноценным recovery manager.

## 7. Post-ExitBootServices

После `ExitBootServices` запрещены обращения к UEFI Boot Services, их allocator, console APIs и UEFI filesystem protocols.

Все данные, необходимые для Linux handoff, должны быть подготовлены заранее и находиться в памяти, которая остаётся доступной kernel.

## 8. Ответственность загрузчика

`luna-boot` владеет только UEFI boot boundary, selection, fallback и handoff.

Он не владеет:

- UserSession;
- application lifecycle;
- Bundle management;
- graphical desktop;
- обычным service management;
- пользователями и их данными.

## 9. Связанные контракты

Подробные draft contracts находятся в:

```text
docs/contracts/SYSTEM-IMAGE-CONTRACT.md
docs/contracts/KERNEL-CONTRACT.md
docs/contracts/BOOT-STATE-CONTRACT.md
docs/contracts/BOOT-HANDOFF-CONTRACT.md
docs/contracts/FAILURE-RECOVERY-CONTRACT.md
```

Они уточняют детали и до отдельного утверждения имеют статус черновиков.