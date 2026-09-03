# Project Luna — Руководство по сборке

**Статус:** текущая инструкция сборки
**Источник архитектуры:** `docs/ARCHITECTURE.md`

## 1. Подготовка

Нужны:

- Rust stable и `rustup`;
- Cargo targets `x86_64-unknown-linux-musl` и `x86_64-unknown-uefi`;
- `git`, `curl`, `make`, `meson`, `ninja`, `cmake`, `pkg-config`, `python3`, `zig`;
- инструменты ext4/FAT/GPT: `mkfs.ext4`, `mkfs.fat`, `sgdisk`, `mcopy`, `mmd`, `dd`;
- инструменты образов: `mksquashfs`, `cpio`, `gzip`, `file`;
- для UEFI/QEMU: `qemu-system-x86_64` и OVMF;
- статический x86_64 BusyBox для текущего PC image flow.

Ubuntu используется как host/CI environment. Сам Linux kernel собирается из upstream Linux sources.

## 2. Проверка workspace

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## 3. Сборка отдельного crate

```bash
bash tools/build-component.sh luna-system-runtime --release
```

Для другого workspace crate меняется только имя package.

## 4. `luna-boot`

```bash
bash tools/build-luna-boot.sh
```

Результат:

```text
boot/luna-boot/target/x86_64-unknown-uefi/release/luna-boot.efi
```

После сборки UEFI path можно проверить отдельно:

```bash
bash tools/test-luna-boot-ovmf.sh
```

Переменные `OVMF_CODE`, `OVMF_VARS`, `LUNA_TEST_KERNEL`, `LUNA_TEST_INITRD` и `LUNA_TEST_SQUASHFS` должны быть заданы.

## 5. Linux kernel

```bash
bash tools/build-luna-kernel.sh
```

Текущая версия по умолчанию задаётся внутри скрипта. Результат находится в `dist/kernel/`, а `dist/kernel/current/` указывает на выбранную версию.

## 6. Desktop root

```bash
bash tools/build-desktop-root.sh
```

По умолчанию результат:

```text
dist/desktop-root/
```

Затем добавляются Yazi/Luna Files, desktop-службы и финальный `niri-session`:

```bash
LUNA_DESKTOP_ROOT=dist/desktop-root bash tools/build-yazi-payload.sh
LUNA_DESKTOP_ROOT=dist/desktop-root bash tools/prepare-desktop-services.sh
LUNA_DESKTOP_ROOT=dist/desktop-root bash tools/patch-niri-session.sh
```

## 7. Полный PC image

Для обычной разработки лучше использовать один входной скрипт:

```bash
bash tools/build-full-pc-image.sh
```

Он выполняет этапы строго в порядке:

```text
kernel
 ↓
desktop root
 ↓
Yazi + Luna Files
 ↓
desktop services
 ↓
niri-session
 ↓
EFI + SYSTEM + DATA image
```

Результат:

```text
dist/luna-pc.img
```

## 8. Сборка только PC image из уже подготовленных компонентов

Если kernel и desktop root уже готовы:

```bash
LUNA_TEST_KERNEL=dist/kernel/current/bzImage \
BUSYBOX=/path/to/static/busybox \
LUNA_DESKTOP_ROOT=dist/desktop-root \
bash tools/build-pc-image.sh
```

## 9. Проверка результата

Минимальная проверка:

```bash
file dist/luna-pc.img
test -f dist/BUILD-INFO
test -f dist/SHA256SUMS
```

Для UEFI-проверки используйте OVMF-тест `luna-boot`. Полная graphical desktop boot-chain пока не считается доказанной только фактом успешной сборки образа.

## 10. Принцип порядка

Не следует запускать дорогой полный build для исправления локальной ошибки crate. Сначала собирайте и тестируйте самый узкий затронутый слой, затем интеграционный слой, затем полный образ.
