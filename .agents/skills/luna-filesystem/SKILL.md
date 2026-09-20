---
name: luna-filesystem
description: Implement and review Luna storage, logical root, LUNA-SYS, LUNA-DATA, SquashFS, and Btrfs behavior.
---
# Luna Filesystem
LUNA-SYS is ext4 and contains images, cores, kernels, config, and recovery artifacts.
LUNA-DATA is Btrfs and contains `system`, `users`, and `cache`.
A System Image is directly `luna-X.Y.Z.squashfs` with an adjacent TOML manifest.
Recovery DATA is a separate artifact under `LUNA-SYS/recovery/`.
Do not substitute unrelated DATA partitions silently.
Do not use `/run` as whole-system staging or copy the entire System Image into RAM.
Keep `system/state/` and `system/volumes/` distinct durable resource classes.
Drivers and firmware remain separate resource classes.
