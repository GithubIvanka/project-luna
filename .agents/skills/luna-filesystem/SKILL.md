---
name: luna-filesystem
description: Реализовывать хранилище и логический корень Luna согласно контрактам LUNA-SYS, LUNA-DATA, SquashFS и Btrfs.
---
# Файловые системы Luna

LUNA-SYS — ext4 с `images`, `cores`, `kernels`, `config` и ресурсами восстановления. LUNA-DATA — Btrfs: `system` содержит apps, drivers, firmware, libs, config, state, volumes; `users/<user>` содержит home, data, config.

System Image — непосредственно `luna-X.Y.Z.squashfs` с соседним TOML-манифестом. `.lbp` — транспортный бандл, а не System Image. DATA восстановления берётся из `LUNA-SYS/recovery/`; физическая DATA является объектом ремонта.

Не подставляйте посторонний раздел DATA. Собирайте логический корень из разрешённых ресурсов; не копируйте весь образ в RAM и не используйте `/run` для подготовки всего корня. `system/state/` и `system/volumes/` обязательны; драйверы и прошивки — отдельные классы ресурсов.
