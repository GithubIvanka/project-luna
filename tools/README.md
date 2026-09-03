# Инструменты разработки Project Luna

Все скрипты запускаются из корня репозитория. Перед запуском убедитесь, что установлены необходимые инструменты, перечисленные в документации соответствующего компонента.

## Сборка отдельного workspace-компонента

```bash
tools/build-component.sh luna-system-runtime --release
```

Скрипт является тонкой оболочкой над `cargo build -p <crate>` и не скрывает дополнительные архитектурные шаги.

## Сборка `luna-boot`

```bash
tools/build-luna-boot.sh
```

Результат:

```text
boot/luna-boot/target/x86_64-unknown-uefi/release/luna-boot.efi
```

## Тест `luna-boot` в QEMU/OVMF

```bash
export OVMF_CODE=/usr/share/OVMF/OVMF_CODE_4M.fd
export OVMF_VARS=/usr/share/OVMF/OVMF_VARS_4M.fd
export LUNA_TEST_KERNEL=/path/to/bzImage
export LUNA_TEST_INITRD=/path/to/initramfs.img
export LUNA_TEST_SQUASHFS=/path/to/luna-test.squashfs
tools/test-luna-boot-ovmf.sh
```

Подробные предпосылки и порядок действий находятся в `boot/luna-boot/README.md`.

## Полная сборка PC-образа

Для воспроизводимой последовательной сборки kernel + desktop root + Yazi/Luna Files + системных desktop-служб + EFI/SYSTEM/DATA image:

```bash
tools/build-full-pc-image.sh
```

Порядок выполнения:

```text
Linux kernel
    ↓
desktop root
    ↓
Yazi + Luna Files
    ↓
NetworkManager / BlueZ / PipeWire / WirePlumber / D-Bus / UDisks2
    ↓
финальный niri-session
    ↓
EFI + SYSTEM + DATA PC image
```

Результат по умолчанию:

```text
dist/luna-pc.img
```

## Низкоуровневые этапы полной сборки

Они могут запускаться отдельно:

```bash
tools/build-luna-kernel.sh
tools/build-desktop-root.sh
LUNA_DESKTOP_ROOT=dist/desktop-root tools/build-yazi-payload.sh
LUNA_DESKTOP_ROOT=dist/desktop-root tools/prepare-desktop-services.sh
LUNA_DESKTOP_ROOT=dist/desktop-root tools/patch-niri-session.sh
LUNA_TEST_KERNEL=dist/kernel/current/bzImage \
LUNA_DESKTOP_ROOT=dist/desktop-root \
tools/build-pc-image.sh
```

Раздельный запуск полезен при разработке конкретного слоя: не требуется пересобирать остальные слои без необходимости.

## Сборка и установка образа

```bash
sudo tools/install-pc-image.sh dist/luna-pc.img /dev/<whole-disk> --yes
```

Операция разрушительная и требует буквального подтверждения `ERASE-LUNA`.

## Правило документации

Каждый компилируемый компонент должен иметь собственный документ в `docs/architecture/components/` с назначением, владельцем, зависимостями, контрактом, инструментами, точными командами сборки, порядком действий, результатом и проверкой. Необходимые автоматические процедуры должны иметь соответствующий скрипт в `tools/`.
