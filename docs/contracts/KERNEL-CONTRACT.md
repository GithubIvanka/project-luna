# Контракт Linux kernel

**Статус:** Accepted / реализация в процессе  
**Дата:** 2026-09-08

## 1. Граница

Kernel является независимым от System Image versioned артефактом. Он хранится в `SYSTEM/kernels/<kernel-id>/` и выбирается только с учётом совместимости, объявленной для выбранного System Image.

Kernel является частью Luna boot chain и должен уметь запускать `luna-init` без отдельного initramfs userspace слоя.

## 2. Формат для x86_64 PC

Текущий boot path использует стандартный Linux `arch/x86/boot/bzImage`. `luna-boot.efi` реализует Linux x86_64 boot protocol.

Luna не требует EFI stub как отдельного userspace слоя.

## 3. Luna kernel boot model

Целевая последовательность:

```text
luna-boot.efi
    ↓
Linux bzImage
    ↓
Luna kernel early boot
    ↓
luna-init (PID 1)
    ↓
luna-system-runtime
```

Kernel configuration и небольшая Luna-specific kernel integration обеспечивают непосредственный запуск `luna-init`.

Классическая схема `kernel → initramfs → switch_root/pivot_root → second init` не является целевой архитектурой Luna.

## 4. Direct initial userspace

`luna-boot` загружает отдельно собранный статический `luna-init` ELF в boot-reserved physical memory. `LunaBootHandoffV1` передаёт kernel физический адрес, размер и digest этого объекта через `LUNA_INIT_IMAGE`.

Kernel валидирует границы, digest и ELF program headers, затем использует Luna-specific direct-userspace path для запуска этого образа как первого userspace процесса/PID 1.

Допускается внутренний kernel-only file/object wrapper вокруг memory range исключительно для повторного использования существующего ELF loader. Он не должен быть представлен userspace как filesystem и не является root layer.

## 5. Boot-critical built-ins

Все kernel capabilities, без которых поддерживаемый Luna boot profile не может обнаружить SYSTEM/DATA, получить доступ к boot storage, прочитать необходимую файловую систему, получить доступ к выбранному System Image и запустить `luna-init`, должны быть built-in (`CONFIG_*=y`).

Категории оцениваются по реальной boot dependency closure, а не по фиксированному заранее списку. В зависимости от поддерживаемой платформы сюда могут входить:

- PCI и соответствующие bus/controller drivers;
- storage-controller drivers;
- NVMe/SATA/USB storage support, когда это требуется boot profile;
- GPT/partition support;
- ext4;
- SquashFS;
- необходимые crypto/verification primitives;
- необходимые input/display/console primitives для выбранного boot profile;
- firmware, необходимое до появления нормального userspace.

Linux допускает встраивание firmware непосредственно в kernel через `CONFIG_EXTRA_FIRMWARE` и `CONFIG_EXTRA_FIRMWARE_DIR`, в том числе когда firmware требуется для доступа к boot device без initramfs. citeturn289743search7

## 6. Loadable modules

`CONFIG_MODULES=y` остаётся допустимым и ожидаемым для необязательных post-boot drivers.

Важно различать:

```text
boot-critical functionality → built-in
optional runtime functionality → module
```

Отсутствие initramfs не означает запрет kernel modules. Оно означает, что ранний boot не зависит от загрузки модулей из initramfs.

## 7. Kernel artifact layout

Kernel и его module set образуют одну versioned artifact identity.

Каноническое направление хранения:

```text
SYSTEM/kernels/
└── <kernel-id>/
    ├── bzImage
    ├── kernel.toml
    └── modules/
        └── lib/modules/<kernel-release>/...
```

`kernel.toml` описывает artifact identity, release, architecture, compatibility metadata и module-set identity.

Модульный набор не является глобальным shared directory для всех kernel versions.

## 8. Module compatibility

Module set должен быть собран против конкретного kernel configuration/release и не должен автоматически считаться совместимым с другим kernel только по имени.

Для shared signing infrastructure необходимо учитывать kernel/module version information; Linux поддерживает kernel-side module signature verification и `CONFIG_MODVERSIONS`.

## 9. Kernel metadata

Для kernel должны быть доступны:

- artifact identity;
- kernel release/version;
- architecture;
- format (`bzImage` для x86_64 PC);
- kernel digest;
- module-set identity/digest;
- compatibility metadata;
- сведения, необходимые `luna-boot` для безопасного выбора.

Точная TOML-схема `kernel.toml` — отдельный контракт и не должна смешиваться с boot handoff ABI.

## 10. Luna Boot Handoff

`luna-boot` передаёт Luna-specific boot context через `LunaBootHandoffV1`, связанный с Linux x86 `setup_data`. Linux x86 boot protocol определяет `setup_data` как extensible linked-list boot data mechanism. citeturn893705search0

Kernel должен валидировать Handoff до запуска initial userspace и сделать проверенный `LunaBootContext` доступным `luna-init`.

## 11. Command line

Kernel command line остаётся частью Linux boot protocol для kernel parameters и диагностики.

Luna-specific storage/image identity не должна зависеть от `luna.system_device`, `luna.data_device` или `luna.system_image` строкового parsing в production boot path.

## 12. Kernel/root responsibility

Kernel не должен превращать System Image в долгоживущий root filesystem только потому, что Luna использует SquashFS.

Целевая ответственность:

```text
kernel
    → hardware + kernel subsystems
    → direct initial userspace launch

luna-init
    → physical Luna resource knowledge
    → System Environment construction
    → logical root construction
```

System Image остаётся immutable source для `luna-init` и дальнейшей hydration/materialization модели.

## 13. Lifecycle

```text
publish
  ↓
validate
  ↓
install
  ↓
compatible with image(s)
  ↓
activate
  ↓
boot confirmation
  ↓
retention eligibility
```

Kernel update не переписывает System Image. System Image update не требует замены kernel, если существующее kernel остаётся совместимым.

## 14. Configuration policy

Luna поддерживает version-adapted kernel configuration policy. Нельзя предполагать, что `.config` одной версии kernel буквально переносим в другую: новые kernel releases могут добавлять или переименовывать Kconfig symbols, поэтому конфигурация должна проверяться и адаптироваться для каждой версии kernel. citeturn517280search0

Текущая repository policy: `kernel/luna-x86_64.config`.

## 15. Open implementation work

- define exact kernel manifest schema;
- define supported boot-profile driver closure;
- implement Luna kernel integration for direct `luna-init` launch;
- build/test required drivers as built-in;
- package version-matched module trees under `SYSTEM/kernels/<kernel-id>/modules`;
- define module signing and trust policy;
- validate Handoff and `LUNA_INIT_IMAGE` before initial userspace launch;
- integrate kernel artifact discovery with `luna-boot`.
