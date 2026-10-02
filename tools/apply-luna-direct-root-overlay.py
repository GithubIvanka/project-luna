#!/usr/bin/env python3
from pathlib import Path
import sys

if len(sys.argv) != 2:
    raise SystemExit("usage: apply-luna-direct-root-overlay.py <kernel-source>")

root = Path(sys.argv[1])
do_mounts = root / "init/do_mounts.c"
init_c = root / "fs/init.c"
init_syscalls = root / "include/linux/init_syscalls.h"

def once(path: Path, old: str, new: str, marker: str) -> None:
    text = path.read_text()
    if marker in text:
        return
    if old not in text:
        raise SystemExit(f"insertion point not found in {path}: {old!r}")
    path.write_text(text.replace(old, new, 1))

once(
    init_syscalls,
    """int __init init_mount(const char *dev_name, const char *dir_name,
		const char *type_page, unsigned long flags, void *data_page);
""",
    """int __init init_mount(const char *dev_name, const char *dir_name,
		const char *type_page, unsigned long flags, void *data_page);
int __init init_set_root(const char *filename);
""",
    "int __init init_set_root(const char *filename);",
)

once(
    init_c,
    "int __init init_umount(const char *name, int flags)\n",
    r"""int __init init_set_root(const char *filename)
{
	struct path path;
	int error;

	error = kern_path(filename, LOOKUP_FOLLOW | LOOKUP_DIRECTORY, &path);
	if (error)
		return error;

	/*
	 * Mounting a filesystem on "/" creates a covering child mount above the
	 * bootstrap root. Install that covering mount as the process root without
	 * calling chroot(2) or pivot_root(2).
	 */
	while (follow_down_one(&path))
		;

	error = path_permission(&path, MAY_EXEC | MAY_CHDIR);
	if (!error) {
		set_fs_root(current->fs, &path);
		set_fs_pwd(current->fs, &path);
	}
	path_put(&path);
	return error;
}

int __init init_umount(const char *name, int flags)
""",
    "int __init init_set_root(const char *filename)",
)

once(
    do_mounts,
    '__setup("root=", root_dev_setup);\n',
    r'''__setup("root=", root_dev_setup);

static int luna_direct_root;
static int __init luna_direct_root_setup(char *str)
{
	if (kstrtoint(str, 0, &luna_direct_root))
		return 0;
	return 1;
}

__setup("luna_direct_root=", luna_direct_root_setup);
''',
    '__setup("luna_direct_root=", luna_direct_root_setup);',
)

once(
    do_mounts,
    "void __init mount_root(char *root_device_name)\n{\n",
    r'''static int __init do_mount_root_direct(const char *name, const char *fs,
					   const int flags, const void *data)
{
	struct super_block *s;
	struct path root;
	char *data_page = NULL;
	int ret;

	if (data) {
		data_page = kmalloc(PAGE_SIZE, GFP_KERNEL);
		if (!data_page)
			return -ENOMEM;
		strscpy_pad(data_page, data, PAGE_SIZE);
	}

	ret = init_mount(name, "/", fs, flags, data_page);
	if (ret)
		goto out;

	ret = init_set_root("/");
	if (ret)
		goto out;

	get_fs_root(current->fs, &root);
	s = root.dentry->d_sb;
	ROOT_DEV = s->s_dev;
	pr_info("Luna: Mounted root (%s) directly on / device %u:%u.\n",
		s->s_type->name, MAJOR(ROOT_DEV), MINOR(ROOT_DEV));
	path_put(&root);

out:
	kfree(data_page);
	return ret;
}

static void __init mount_root_direct(char *root_device_name)
{
	char *fs_names = kmalloc(PAGE_SIZE, GFP_KERNEL);
	char *p;
	int num_fs, i;
	int err;

	if (!fs_names)
		panic("VFS: Unable to mount root fs directly: not enough memory");

	/*
	 * Resolve PARTUUID/PARTLABEL/etc. with Linux's existing early lookup
	 * mechanism. Do not invent a second metadata resolver in the experiment.
	 */
	if (ROOT_DEV == 0) {
		err = early_lookup_bdev(root_device_name, &ROOT_DEV);
		if (err) {
			pr_emerg("VFS: direct root lookup failed for %s: %d.\n",
				 root_device_name, err);
			kfree(fs_names);
			panic("VFS: direct root lookup failed for %s", root_device_name);
		}
	}

	err = create_dev("/dev/root", ROOT_DEV);
	if (err < 0) {
		kfree(fs_names);
		panic("VFS: unable to create /dev/root: %d", err);
	}

	if (root_fs_names)
		num_fs = split_fs_names(fs_names, PAGE_SIZE);
	else
		num_fs = list_bdev_fs_names(fs_names, PAGE_SIZE);

	for (i = 0, p = fs_names; i < num_fs; i++, p += strlen(p) + 1) {
		if (!*p)
			continue;
		err = do_mount_root_direct("/dev/root", p, root_mountflags,
					   root_mount_data);
		if (!err) {
			kfree(fs_names);
			return;
		}
	}

	pr_emerg("VFS: Unable to mount root fs directly on namespace root.\n");
	printk_all_partitions();
	kfree(fs_names);
	panic("VFS: direct root mount failed for %s", root_device_name);
}

void __init mount_root(char *root_device_name)
{
''',
    "static void __init mount_root_direct(char *root_device_name)",
)

once(
    do_mounts,
    "\tinitrd_load();\n",
    r'''    if (luna_direct_root) {
        /*
         * The block layer and partition tables must be registered before
         * PARTUUID/PARTLABEL can be resolved.
         */
        if (root_wait)
            wait_for_root(saved_root_name);
        devtmpfs_mount();
        mount_root_direct(saved_root_name);
        /*
         * The direct root mount covers the bootstrap root. Establish a normal
         * devtmpfs at /dev on that mounted root for Alpine userspace.
         */
        devtmpfs_mount();
        return;
    }

	initrd_load();
''',
    "if (luna_direct_root) {",
)

print("direct-root overlay applied:", root)
