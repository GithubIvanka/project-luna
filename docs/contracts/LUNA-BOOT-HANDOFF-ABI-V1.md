# Контракт LunaBootHandoffV1

Handoff объявляется `luna-boot.efi` через Linux x86 `setup_data`.

Обязательные записи:

```text
SYSTEM_PARTITION
DATA_PARTITION
SYSTEM_IMAGE
KERNEL_IDENTITY
LUNA_INIT_IMAGE
BOOT_MODE
BOOT_STATE
```

`LUNA_INIT_IMAGE` содержит физический адрес, точный размер в байтах и BLAKE3-256 digest выбранного ELF `.init`.

Kernel проверяет границы, структуру записей, наличие обязательных записей, память и целостность init image, а также ELF constraints.

Проверенные bytes handoff передаются initial userspace process как read-only FD 3 с offset zero.

Изменение ABI требует явной ревизии контракта.
