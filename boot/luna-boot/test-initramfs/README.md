# Test initramfs

This is a temporary bring-up initramfs used only to verify that `luna-boot`
can load an initramfs and Linux can execute `/init`.

It intentionally does not mount a Luna System Image. The real System Image
will remain a SquashFS filesystem image (`luna-X.Y.Z.squashfs`) and will be
integrated later.

## Build

From this directory, with `cpio` and `gzip` installed:

```sh
chmod +x init
mkdir -p root
cp -a init root/init
chmod +x root/init
(
    cd root
    find . -print | cpio -o -H newc
) | gzip -n > ../initramfs-test.img
rm -rf root
```

Copy the resulting `initramfs-test.img` to the root of the mounted Luna
`system` partition at `boot/initramfs-test.img`.
