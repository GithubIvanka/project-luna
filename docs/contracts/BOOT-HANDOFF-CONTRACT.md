# Контракт передачи управления из `luna-boot`

**Статус:** Accepted / реализуется по ABI v1.  
**Дата:** 2026-09-08

## 1. Цель

После работы UEFI-загрузчика Linux kernel должен получить самодостаточный Luna boot context и запустить `luna-init` напрямую, без отдельного initramfs userspace слоя.

Целевая цепочка:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Luna Linux kernel
  ↓
luna-init (PID 1)
  ↓
luna-system-runtime
```

## 2. Основной transport

Luna-specific boot data передаётся через Linux x86 `setup_data` как запись `LunaBootHandoffV1`, размещённую в физической памяти до `ExitBootServices`.

Linux x86 boot protocol предоставляет `setup_data` именно как расширяемый механизм передачи boot parameters; базовый `boot_params` ограничен 4096 байтами, а `setup_data` позволяет расширять его без изменения фиксированной структуры. citeturn880881search1turn880881search2

Handoff не является файлом и не хранится на SYSTEM, DATA или EFI.

## 3. Что передаётся

Обязательные Luna-specific records v1:

- `SYSTEM_PARTITION` — GPT disk GUID + partition GUID;
- `DATA_PARTITION` — GPT disk GUID + partition GUID;
- `SYSTEM_IMAGE` — семейство, версия, имя файла, manifest identity и image digest;
- `KERNEL_IDENTITY` — identity/version running kernel и digest выбранного kernel artifact;
- `BOOT_MODE` — normal/detailed/recovery/factory/external;
- `BOOT_STATE` — минимальный контекст текущей boot attempt.

Актуальная бинарная структура и record encoding определены в `docs/contracts/LUNA-BOOT-HANDOFF-ABI-V1.md`.

## 4. Устройства и идентичность

Luna не привязывает boot contract к Linux device names (`/dev/sda`, `/dev/sdb`, `/dev/nvme...`).

SYSTEM и DATA идентифицируются стабильными GPT identities. `luna-init` после старта kernel разрешает эти identities в фактические Linux block devices.

## 5. System Image

`luna-boot` выбирает System Image до запуска kernel и проверяет его manifest/совместимость.

Handoff передаёт identity выбранного image и ожидаемый content digest. `luna-init` не должен выбирать другой image только потому, что найден другой файл с похожим именем.

System Image остаётся immutable source. Он не становится физическим `/`.

## 6. Kernel

Kernel является отдельным versioned artifact. Handoff содержит его identity для provenance и проверки boot attempt; передавать сам kernel в handoff не требуется, поскольку kernel уже исполняется.

Kernel selection производится `luna-boot` до handoff и всегда учитывает compatibility relation с System Image.

## 7. Hardware information boundary

Generic hardware/platform data остаётся в стандартном Linux boot protocol и в данных, которые Linux kernel получает от firmware/bootloader.

Luna Handoff не дублирует ACPI, E820 или другие kernel-owned hardware descriptions. `boot_params` уже содержит поля для platform information, а `setup_data` используется для расширений. citeturn856784search2turn880881search1

## 8. Memory ownership

Handoff storage выделяется `luna-boot` до `ExitBootServices`.

Память handoff должна быть сохранена как boot-reserved до момента, когда Luna kernel integration завершит обработку объекта. Нельзя полагаться на случайное сохранение адреса в свободной RAM.

После `ExitBootServices` `luna-boot` не использует Boot Services allocator, UEFI filesystem protocols или console APIs.

## 9. Command line

Kernel command line остаётся допустимым transport для стандартных Linux kernel parameters и временной диагностики.

Однако Luna не использует `luna.system_device`, `luna.data_device` и `luna.system_image` как основной production ABI после внедрения Handoff v1.

Таким образом, Luna-specific boot state является структурированным объектом, а не набором строковых параметров.

## 10. Initial userspace

Luna kernel integration должна сделать validated `LunaBootHandoffV1` доступным `luna-init` до его запуска.

`luna-init` получает как минимум:

```text
SYSTEM identity
DATA identity
selected image identity + digest
running kernel identity
boot attempt id
boot mode
boot state context
```

## 11. Initramfs policy

В production boot path Luna **не использует отдельный initramfs userspace**.

`luna-boot` не загружает `luna-init` как отдельный initramfs filesystem. Kernel должен запускать `luna-init` как непосредственный initial userspace entry.

Классическая двухфазная схема `kernel → initramfs → real root → switch_root/pivot_root → second init` не является целевой архитектурой Luna.

## 12. Kernel built-in policy

Чтобы отказаться от initramfs dependency для early boot, все драйверы, bus support, block/filesystem support, crypto primitives и другие kernel features, без которых поддерживаемый Luna boot profile не может:

```text
discover SYSTEM
  ↓
access DATA when required
  ↓
read required filesystem
  ↓
access System Image
  ↓
start luna-init
```

должны быть встроены в kernel (`CONFIG_*=y`).

Остальные optional drivers могут быть loadable modules.

Built-in firmware является допустимым дополнением, когда устройство требует firmware на ранней стадии и filesystem lookup создал бы нежелательную раннюю userspace зависимость. Linux поддерживает встроенное firmware через `CONFIG_EXTRA_FIRMWARE` и `CONFIG_EXTRA_FIRMWARE_DIR`. citeturn856784search0

## 13. Kernel modules

Loadable modules принадлежат конкретной версии kernel и не являются глобальным shared pool.

Каноническое направление хранения:

```text
SYSTEM/kernels/
└── <kernel-id>/
    ├── bzImage
    ├── kernel.toml
    └── modules/
        └── lib/modules/<kernel-release>/...
```

Такой layout позволяет связать kernel artifact и его module set одной versioned identity. После построения logical root module tree может быть подключён/материализован в обычную Linux filesystem hierarchy.

Если loadable modules разрешены, production policy может требовать kernel-side signature enforcement; Linux поддерживает подписи модулей и принудительное отклонение неподписанных/невалидных модулей. citeturn856784search4

## 14. Ошибки

Handoff считается недействительным при:

- неподдерживаемом major ABI;
- неверном magic;
- повреждённом checksum;
- выходе record за `total_size`;
- integer overflow;
- отсутствующем обязательном record;
- невозможной/несогласованной identity.

При invalid handoff `luna-init` не должен молча переходить к старому строковому fallback discovery.

## 15. Связанные документы

```text
docs/contracts/LUNA-BOOT-HANDOFF-ABI-V1.md
docs/contracts/KERNEL-CONTRACT.md
docs/contracts/LUNA-INIT-CONTRACT.md
docs/contracts/BOOT-STATE-CONTRACT.md
docs/contracts/FAILURE-RECOVERY-CONTRACT.md
```
