# luna-boot development rules

The bootloader follows `docs/ARCHITECTURE.md` and `docs/architecture/BOOT-PATH.md`.

Do not introduce a new boot layer, runtime component, boot profile or alternate initialization architecture without explicit approval.

Keep UEFI responsibilities inside `luna-boot`: discovery, selection, menu, memory preparation and handoff. Do not move userspace runtime policy into the bootloader merely for convenience.

Do not treat scripts or a successful compile as proof of end-to-end bootability.

Normal boot must remain quiet and direct. `B` is the explicit exception that opens Boot Menu.

When changing the boot ABI, update the ABI contract before implementation and test both producer and consumer.
