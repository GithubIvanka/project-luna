# Luna development tools

## Build a PC image

```bash
tools/build-pc-image.sh
```

The default output is `dist/luna-pc.img`.

The builder auto-detects `/boot/vmlinuz-*` and static BusyBox when available.
Set `LUNA_TEST_KERNEL` and `BUSYBOX` to override detection.

## Install a PC image

```bash
sudo tools/install-pc-image.sh dist/luna-pc.img /dev/<whole-disk> --yes
```

This is destructive and always asks for the literal `ERASE-LUNA` confirmation.

For the full procedure see `docs/development/PC-BUILD.md`.
