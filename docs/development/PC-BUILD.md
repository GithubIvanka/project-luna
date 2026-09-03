# Project Luna — сборка PC-образа

Это текущий воспроизводимый development image для Project Luna. Он создаёт один UEFI/GPT-образ и не изменяет диск хоста.

## Результат

`tools/build-pc-image.sh` создаёт:

```text
dist/luna-pc.img
```

Разметка:

```text
EFI     128 MiB
SYSTEM  384 MiB
DATA    512 MiB
```

Development image использует принятую физическую модель Luna:

```text
EFI
SYSTEM
DATA
```

SWAP в development image намеренно отсутствует.

## Цепочка загрузки

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
ранний initramfs
  ↓
SYSTEM (read-only)
  ↓
выбранный versioned SquashFS System Image
  ↓
DATA (read-write)
  ↓
luna-init
  ↓
logical /
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
Wayland → niri → Noctalia
```

Здесь нет отдельного `luna-session` компонента и нет штатного TTY login path.

## Требования к хосту

На Debian/Ubuntu-подобном хосте нужны:

```bash
sudo apt install \
  busybox-static \
  cpio \
  dosfstools \
  e2fsprogs \
  gdisk \
  linux-image-amd64 \
  musl-tools \
  mtools \
  squashfs-tools
```

Также нужны Rust stable и `rustup`. Сборщик при необходимости устанавливает targets `x86_64-unknown-linux-musl` и `x86_64-unknown-uefi`.

Автоматически используются:

```text
/boot/vmlinuz-*
/usr/bin/busybox
/bin/busybox
```

Явные пути задаются через `LUNA_TEST_KERNEL` и `BUSYBOX`.

Для OVMF нужны QEMU и отдельный writable variables-файл.

## Сборка

Основной development flow:

```bash
tools/build-pc-image.sh
```

Не следует запускать полный build для исправления локальной ошибки crate. Сначала проверяется затронутый слой, затем интеграционный слой, затем полный image.

## Установка на реальный диск

Установка выполняется отдельно от сборки:

```bash
sudo tools/install-pc-image.sh dist/luna-pc.img /dev/nvme0n1 --yes
```

Команда работает только с целым block device. Перед destructive write installer дополнительно требует подтверждение `ERASE-LUNA` и отказывается от target с mounted filesystems.

Не указывайте раздел вроде `/dev/nvme0n1p1`.

Репозиторий автоматически не записывает образ на физический диск.

## Проверка результата

Минимальная проверка:

```bash
file dist/luna-pc.img
test -f dist/BUILD-INFO
test -f dist/SHA256SUMS
```

Для UEFI-проверки используйте OVMF test path. Успешная сборка image сама по себе не доказывает полный end-to-end graphical boot.

## Текущие ограничения

Это development image, а не production installer.

Финальные inventory и compatibility rules для kernel/System Image, persistent boot-success state, полноценная графическая интеграция, production child-creation primitive и полная device/portal integration ещё требуют отдельной разработки.

Графическая граница остаётся:

```text
luna-system-runtime
  ↓
UserSession
  ↓
Wayland
  ↓
niri
  ↓
Noctalia
```
