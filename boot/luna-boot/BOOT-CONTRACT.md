# luna-boot contract

## Partition identities

The bootloader first verifies the mandatory EFI/`LUNA-SYS` same-disk relationship, then uses the canonical GPT label `LUNA-SYS`. Linux device names are not the stable contract. `LUNA-DATA` may be on the same disk or another disk and is resolved separately through `LUNA-SYS/config/luna-data.toml`.

## LUNA-SYS layout

```text
images/
cores/
kernels/
config/
recovery/
```

The loader reads image/core manifests and kernel artifacts from those locations.

## Direct init

The selected `luna-init` core is loaded as exact bytes into boot-reserved memory and is a mandatory member of the complete `System Image + luna-init + kernel` boot target. It remains a standalone versioned ELF artifact in `LUNA-SYS/cores/`.

## Handoff

`LunaBootHandoffV1` is delivered through Linux x86 `setup_data`. Required records identify the system partition, DATA partition, System Image, kernel, init image, boot mode and boot state.

## Boot attempt

`LunaBootAttempt` is written once before `ExitBootServices`. It is cleared only after semantic system boot success by `luna-system-runtime`.

## Failure

Soft image/bootstrap failures may use another compatible image with the loaded kernel. Kernel panic requires reboot and a new bootloader selection based on durable state.
