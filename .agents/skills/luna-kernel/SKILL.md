---
name: luna-kernel
description: Работать с Linux-overlay Luna, прямым запуском luna-init, handoff и оборудованием Alpha.
---
# Ядро Luna

Сохраняйте небольшую явную интеграцию Luna с Linux. Проверяйте `LunaBootHandoffV1`, диапазоны памяти, ограничения init ELF и digest; используйте существующие `components/kernel/rust/luna_boot.rs` и `luna_exec.rs`, пока доказательства не требуют изменения.

Сохраняйте прямой запуск начального пользовательского процесса из переданного в памяти luna-init. Приоритет Alpha: Intel UHD 630/i915, AX200 Wi-Fi/Bluetooth, r8169, Intel HDA audio, USB xHCI, NVMe и SATA.

Необходимые ранние драйверы для Alpha предпочтительно встраивать в ядро. Проверяйте Btrfs, SquashFS, EFI/GPT и необходимые namespaces/cgroups/security-функции по текущим контрактам и конфигурации; не усложняйте управление модулями заранее.
