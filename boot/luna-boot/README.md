# `luna-boot`

`luna-boot` — самостоятельный UEFI-загрузчик Project Luna. Он намеренно находится отдельно от обычного Cargo workspace userspace.

## Текущая архитектура

```text
UEFI
  ↓
luna-boot.efi
  ↓
GPT → SYSTEM → ext4
  ↓
System Image + совместимое Linux bzImage
  ↓
boot_params + E820 + initramfs
  ↓
ExitBootServices
  ↓
identity paging + вход x86_64 Linux
  ↓
Linux
```

Меню загрузки не имеет таймера. `luna-boot` один раз проверяет очередь ввода UEFI при запуске: если в очереди есть `B`, открывается меню; иначе загрузка продолжается без искусственной задержки.

Сам System Image загрузчик не интерпретирует. Он остаётся файлом `*.squashfs`; ранний userspace Linux отвечает за построение логического корня.

## Граница ответственности

`luna-boot` отвечает за:

- UEFI boot flow;
- обнаружение SYSTEM и boot-метаданных;
- выбор совместимой пары System Image + kernel;
- Boot Menu;
- boot-time fallback;
- передачу boot-параметров Linux.

Он не владеет `UserSession`, application lifecycle, DATA management или обычным userspace runtime.

## Сборка

### Необходимые инструменты

Минимально требуются:

- Rust stable через `rustup`;
- `cargo`;
- target `x86_64-unknown-uefi`;
- для OVMF-теста: `qemu-system-x86_64`, `sgdisk`, `mkfs.ext4`, `mkfs.fat`, `mformat`, `mmd`, `mcopy`, `dd`.

На Ubuntu также должны быть доступны инструменты UEFI/EFI и QEMU из соответствующих системных пакетов.

### Рекомендуемый способ сборки

Из корня репозитория:

```bash
bash tools/build-luna-boot.sh
```

Скрипт:

1. проверяет `cargo` и `rustup`;
2. устанавливает `x86_64-unknown-uefi`, если target отсутствует;
3. переходит в `boot/luna-boot`;
4. выполняет release-сборку;
5. проверяет наличие итогового `.efi`.

Результат:

```text
boot/luna-boot/target/x86_64-unknown-uefi/release/luna-boot.efi
```

Ручная эквивалентная команда:

```bash
cd boot/luna-boot
cargo build --release --target x86_64-unknown-uefi
```

### Проверка

```bash
file boot/luna-boot/target/x86_64-unknown-uefi/release/luna-boot.efi
```

Для полноценного OVMF-теста:

```bash
export OVMF_CODE=/usr/share/OVMF/OVMF_CODE_4M.fd
export OVMF_VARS=/usr/share/OVMF/OVMF_VARS_4M.fd
export LUNA_TEST_KERNEL=/path/to/bzImage
export LUNA_TEST_INITRD=/path/to/initramfs.img
export LUNA_TEST_SQUASHFS=/path/to/luna-test.squashfs
bash tools/test-luna-boot-ovmf.sh
```

Скрипт сначала собирает `luna-boot`, затем запускает существующий тест `boot/luna-boot/tests/ovmf/run.sh`.

## Важное ограничение текущего теста

Текущий OVMF bring-up доказывает UEFI/Linux boot path и тестовую загрузку через init, но сам по себе ещё не является доказательством полного production-сценария `System Image → logical root → luna-system-runtime → UserSession → graphical desktop`.
