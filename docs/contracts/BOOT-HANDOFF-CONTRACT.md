# Контракт Boot Handoff

Каноническая граница:

```text
luna-boot.efi
  ↓
Linux boot protocol + LunaBootHandoffV1
  ↓
Linux kernel direct-init path
```

Bootloader подготавливает все данные, которые потребуются после `ExitBootServices`, до выхода из UEFI Boot Services.

Handoff использует физическую identity разделов и артефактов, а не имена Linux device nodes как архитектурный ABI.

`LUNA-SYS` — управляемый ОС системный раздел, связанный с EFI на одном физическом диске. `luna-boot.efi` проверяет эту связь и загружает ОС только из `LUNA-SYS` этого диска. `LUNA-DATA` может находиться на том же или другом физическом диске. Его обычная привязка определяется GUID диска и GUID раздела из `LUNA-SYS/config/luna-data.toml`. Recovery может запускаться без физического DATA.
