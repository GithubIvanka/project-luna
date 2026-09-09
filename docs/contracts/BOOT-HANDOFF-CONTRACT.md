# Контракт передачи управления из `luna-boot`

**Статус:** Accepted / ABI v1 implementation in progress  
**Scope:** `luna-boot.efi` → Linux kernel → `luna-init`

## Целевая цепочка

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

Отдельного initramfs userspace этапа нет.

## Ответственность `luna-boot`

`luna-boot.efi` отвечает за UEFI boundary, обнаружение SYSTEM, discovery System Images и kernels, чтение manifest, выбор совместимой пары, Boot Menu, fallback, подготовку Linux boot context, загрузку kernel и `luna-init`, построение `LunaBootHandoffV1`, `ExitBootServices` и передачу управления kernel.

После `ExitBootServices` загрузчик не использует UEFI Boot Services, UEFI filesystem protocols или UEFI console APIs.

## Handoff transport

Luna-specific boot data передаётся через Linux x86 `setup_data` как один `LunaBootHandoffV1` object. Handoff находится в физической памяти и не является файлом.

Каноническая ABI v1 описана в `docs/contracts/LUNA-BOOT-HANDOFF-ABI-V1.md`.

## Что передаётся

Обязательные v1 records:

- `SYSTEM_PARTITION`;
- `DATA_PARTITION`;
- `SYSTEM_IMAGE`;
- `KERNEL_IDENTITY`;
- `LUNA_INIT_IMAGE`;
- `BOOT_MODE`;
- `BOOT_STATE`.

Handoff содержит identity и digest выбранных артефактов и идентичность физических SYSTEM/DATA partition. Linux device names не являются ABI.

## `luna-init`

Для выбранного System Image `luna-boot` загружает отдельный ELF artifact:

```text
SYSTEM/images/luna-X.Y.Z.init
```

Artifact является memory-resident и boot-reserved. В `LUNA_INIT_IMAGE` передаются физический адрес, размер и BLAKE3-256 digest exact ELF byte range.

Kernel обязан валидировать этот объект и запустить его непосредственно как initial userspace process. `luna-init` становится PID 1.

## Memory ownership

Handoff и загруженный `luna-init` являются boot-reserved objects. Их диапазоны не должны рассматриваться kernel как свободная RAM до завершения соответствующего kernel-side processing.

Kernel должен сохранить исходные bytes `luna-init` до момента успешной загрузки ELF и освобождать backing memory только после того, как она больше не нужна.

## Command line

Kernel command line остаётся для стандартных Linux parameters и временной диагностики.

Production boot не зависит от:

```text
luna.system_device
luna.data_device
luna.system_image
```

Эти значения не являются primary Luna boot ABI.

## System Image

`luna-boot` выбирает конкретный System Image и передаёт его identity/digest через handoff. `luna-init` не должен выбирать другой image из-за случайного совпадения имени.

System Image остаётся immutable source и не становится физическим `/`.

## Hardware boundary

Handoff не дублирует ACPI, E820 или другую generic hardware information. Эта информация передаётся/обнаруживается через стандартный Linux boot path.

## Initramfs policy

Luna production boot **не использует initramfs**.

`luna-init` не транспортируется в cpio/gzip archive и не запускается через отдельный initramfs `/init`.

Целевой путь:

```text
Linux kernel
    ↓
LunaBootHandoffV1
    ↓
memory-resident luna-init ELF
    ↓
luna-init PID 1
```

Любая документация, build script или image builder, создающие `luna-initramfs.img` либо описывающие `kernel → initramfs → luna-init`, считаются устаревшими и должны быть удалены.

## Userspace channel

После валидации handoff kernel предоставляет `luna-init` canonical boot context через:

```text
FD 3 = read-only Luna boot-context
```

FD 3 начинается с offset 0 и содержит сериализованные bytes валидированного `LunaBootHandoffV1`. `luna-init` обязан прочитать context и закрыть FD 3 до создания/запуска обычных дочерних процессов.

## Failure

Повреждённый или неполный handoff, отсутствующий обязательный record, невалидный `LUNA_INIT_IMAGE`, невозможная memory range или failure direct-init startup — это boot failure.

`luna-init` не должен молча переходить на legacy path discovery или выбирать другой image/device, если structured ABI invalid.
