// SPDX-License-Identifier: GPL-2.0
/*
 * Project Luna direct-PID1 launcher.
 *
 * The bootloader supplies both the validated Luna handoff and a standalone
 * luna-init ELF as boot-reserved physical memory. This bridge copies those
 * byte ranges into memory-backed kernel files, installs the handoff as fd 3,
 * and lets Linux's normal kernel_execve() ELF loader create PID 1.
 */

#include <linux/binfmts.h>
#include <linux/err.h>
#include <linux/fdtable.h>
#include <linux/file.h>
#include <linux/fs.h>
#include <linux/init.h>
#include <linux/io.h>
#include <linux/shmem_fs.h>
#include <linux/slab.h>
#include <linux/types.h>

extern bool x86_luna_boot_available(void);
extern u64 x86_luna_init_phys(void);
extern u64 x86_luna_init_size(void);
extern u64 x86_luna_handoff_phys(void);
extern u32 x86_luna_handoff_size(void);

static int luna_copy_phys_to_file(struct file *file, phys_addr_t phys,
                                  size_t size)
{
	void *mapped;
	loff_t pos = 0;
	size_t offset = 0;
	int ret = 0;

	if (!size)
		return -EINVAL;

	mapped = memremap(phys, size, MEMREMAP_WB);
	if (!mapped)
		return -EFAULT;

	while (offset < size) {
		size_t chunk = min_t(size_t, size - offset, MAX_RW_COUNT);
		ssize_t written = kernel_write(file, mapped + offset, chunk, &pos);

		if (written != chunk) {
			ret = written < 0 ? (int)written : -EIO;
			break;
		}
		offset += chunk;
	}

	memunmap(mapped);
	return ret;
}

static int luna_install_handoff_fd(void)
{
	struct file *file;
	int fd;
	int ret;
	phys_addr_t phys = x86_luna_handoff_phys();
	u32 size = x86_luna_handoff_size();

	if (!phys || !size)
		return -EINVAL;

	file = shmem_kernel_file_setup("luna-boot-handoff", size, VM_NORESERVE);
	if (IS_ERR(file))
		return PTR_ERR(file);

	ret = luna_copy_phys_to_file(file, phys, size);
	if (ret) {
		fput(file);
		return ret;
	}

	/* The descriptor is a read-only boot-context channel by contract. */
	file->f_mode &= ~(FMODE_WRITE | FMODE_CAN_WRITE);
	file->f_flags = (file->f_flags & ~O_ACCMODE) | O_RDONLY;
	file->f_pos = 0;

	/* console_on_rootfs() has already consumed descriptors 0, 1 and 2. */
	fd = get_unused_fd_flags(0);
	if (fd < 0) {
		fput(file);
		return fd;
	}
	if (fd != 3) {
		put_unused_fd(fd);
		fput(file);
		return -EMFILE;
	}

	/* fd_install() consumes the file reference. */
	fd_install(3, file);
	return 0;
}

static int __init luna_stage_init_image(void)
{
	struct file *file;
	phys_addr_t phys = x86_luna_init_phys();
	u64 size64 = x86_luna_init_size();
	int ret;

	if (!phys || !size64 || size64 > MAX_LFS_FILESIZE)
		return -EINVAL;

	file = filp_open("/luna-init", O_WRONLY | O_CREAT | O_TRUNC |
				 O_LARGEFILE, 0700);
	if (IS_ERR(file))
		return PTR_ERR(file);

	ret = luna_copy_phys_to_file(file, phys, (size_t)size64);
	fput(file);
	return ret;
}

int __init x86_luna_exec_init(void)
{
	static const char *const argv[] = {
		"/luna-init",
		NULL,
	};
	static const char *const envp[] = {
		"PATH=/bin:/sbin:/usr/bin:/usr/sbin",
		"LUNA_DIRECT_INIT=1",
		NULL,
	};
	int ret;

	if (!IS_ENABLED(CONFIG_RUST))
		return -ENOSYS;
	if (!x86_luna_boot_available())
		return -ENOENT;

	pr_info("Luna: staging memory-resident luna-init for direct PID 1\n");

	ret = luna_install_handoff_fd();
	if (ret) {
		pr_err("Luna: failed to install handoff fd 3: error %d\n", ret);
		return ret;
	}

	ret = luna_stage_init_image();
	if (ret) {
		pr_err("Luna: failed to stage luna-init: error %d\n", ret);
		return ret;
	}

	pr_info("Luna: executing luna-init as PID 1\n");
	ret = kernel_execve("/luna-init", argv, envp);
	pr_err("Luna: kernel_execve(/luna-init) failed: error %d\n", ret);
	return ret;
}
