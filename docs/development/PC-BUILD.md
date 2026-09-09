# Project Luna — сборка PC-образа

Это текущий воспроизводимый development image для Project Luna. Он создаёт один UEFI/GPT-образ и не изменяет диск хоста.

## Результат

`tools/build-pc-image.sh` создаёт:

```text
dist/luna-pc.img
```

Разметка development image:

```text
EFI     128 MiB
SYSTEM  configurable, default 768 MiB
DATA    configurable, default 512 MiB
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
luna-init (PID 1, direct memory-resident launch)
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
Wayland → niri → Noctalia
```

`luna-init` получает `LunaBootHandoffV1` через фиксированный FD 3 и не выполняет файловый bootstrap. SYSTEM и DATA монтируются уже следующим userspace-слоем.

## Требования к хосту

На Debian/Ubuntu-подобном хосте нужны:

```bash
sudo apt install \
  dosfstools \
  e2fsprogs \
  gdisk \
  musl-tools \
  mtools \
  squashfs-tools
```

Также нужны Rust stable и `rustup`. Сборщик при необходимости устанавливает targets `x86_64-unknown-linux-musl` и `x86_64-unknown-uefi`.

Ядро для теста задаётся через `LUNA_TEST_KERNEL`. Для production-like development image также требуется подготовленный `LUNA_DESKTOP_ROOT`.

Для UEFI-проверки нужны QEMU/OVMF и отдельный writable variables-файл.

## Сборка

Основной development flow:

```bash
tools/build-pc-image.sh
```

Сборщик помещает в SYSTEM:

```text
images/
├── luna-X.Y.Z.squashfs
├── luna-X.Y.Z.toml
└── luna-X.Y.Z.init

kernels/
└── <kernel-version>/
    └── bzImage
```

`luna-X.Y.Z.squashfs` является самим System Image; `.init` является отдельным ELF64-артефактом `luna-init` и не является initramfs.

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

Первый прямой userspace milestone проверяет именно переход:

```text
UEFI → luna-boot → kernel → luna-init(PID 1)
```

Полный запуск `luna-system-runtime`, persistent boot-success state, production child/process policy и графическая интеграция остаются следующими слоями разработки.
