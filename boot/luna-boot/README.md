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
System Image manifest + совместимый Linux bzImage
  ↓
boot_params + E820 + LunaBootHandoffV1
  ↓
ExitBootServices
  ↓
identity paging + вход x86_64 Linux
  ↓
Luna Linux kernel
  ↓
luna-init (PID 1)
```

`luna-boot` не загружает отдельный initramfs userspace. Luna kernel должен содержать boot-critical driver/filesystem/crypto support, необходимый для непосредственного запуска `luna-init`.

Меню загрузки не имеет таймера. `luna-boot` один раз проверяет очередь ввода UEFI при запуске: если в очереди есть `B`, открывается меню; иначе загрузка продолжается без искусственной задержки.

Сам System Image загрузчик не интерпретирует как Linux root. Он остаётся файлом `*.squashfs`; `luna-init` использует его как immutable source для построения System Environment.

## Luna Boot Handoff

Luna-specific boot state передаётся как `LunaBootHandoffV1` через Linux x86 `setup_data`.

Handoff находится в выделенной физической памяти и не является файлом.

В v1 передаются:

- identity SYSTEM partition;
- identity DATA partition;
- selected System Image identity and digest;
- running kernel identity and digest;
- boot mode;
- boot attempt/state context.

Полный ABI:

```text
docs/contracts/LUNA-BOOT-HANDOFF-ABI-V1.md
```

## Граница ответственности

`luna-boot` отвечает за:

- UEFI boot flow;
- обнаружение SYSTEM и boot-метаданных;
- выбор совместимой пары System Image + kernel;
- Boot Menu;
- boot-time fallback;
- Linux boot-protocol setup;
- Luna Boot Handoff;
- `ExitBootServices`;
- передачу управления Linux kernel.

Он не владеет `luna-init` runtime supervision, UserSession, application lifecycle, DATA management или обычным userspace runtime.

## Сборка

### Необходимые инструменты

Минимально требуются:

- Rust stable через `rustup`;
- `cargo`;
- target `x86_64-unknown-uefi`;
- для OVMF-теста: `qemu-system-x86_64`, `sgdisk`, `mkfs.ext4`, `mkfs.fat`, `mformat`, `mmd`, `mcopy`, `dd`.

### Рекомендуемый способ сборки

Из корня репозитория:

```bash
bash tools/build-luna-boot.sh
```

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

Для OVMF bring-up и полноценной no-initramfs интеграции сценарий тестирования должен использовать Luna kernel и SYSTEM/DATA test image; отдельный `LUNA_TEST_INITRD` больше не является частью целевого boot contract.

## Важное ограничение текущего теста

Существующий OVMF bring-up доказывает UEFI/Linux handoff и тестовую загрузку, но ещё не является доказательством production-сценария `luna-boot → Luna kernel → luna-init PID 1 → luna-system-runtime → UserSession → graphical desktop`.
