#!/usr/bin/env bash
set -euo pipefail

SRC="$1"
REPO_ROOT="$2"

RUST_SRC="${REPO_ROOT}/components/kernel/rust/luna_boot.rs"
EXEC_SRC="${REPO_ROOT}/components/kernel/rust/luna_exec.rs"
RUST_DST="${SRC}/arch/x86/kernel/luna_boot.rs"
EXEC_DST="${SRC}/arch/x86/kernel/luna_exec.rs"
MAKEFILE="${SRC}/arch/x86/kernel/Makefile"
SETUP_C="${SRC}/arch/x86/kernel/setup.c"
INIT_C="${SRC}/init/main.c"
EXEC_C="${SRC}/fs/exec.c"
BINFMT_H="${SRC}/include/linux/binfmts.h"

for file in "$RUST_SRC" "$EXEC_SRC" "$MAKEFILE" "$SETUP_C" "$INIT_C" "$EXEC_C" "$BINFMT_H"; do
    test -f "$file" || { echo "missing kernel overlay input: $file" >&2; exit 1; }
done

if test -e "$RUST_DST" && ! cmp -s "$RUST_SRC" "$RUST_DST"; then
    echo "refusing to overwrite existing different $RUST_DST" >&2
    exit 1
fi
if test -e "$EXEC_DST" && ! cmp -s "$EXEC_SRC" "$EXEC_DST"; then
    echo "refusing to overwrite existing different $EXEC_DST" >&2
    exit 1
fi
test -e "$RUST_DST" || cp "$RUST_SRC" "$RUST_DST"
test -e "$EXEC_DST" || cp "$EXEC_SRC" "$EXEC_DST"

grep -Fq 'obj-$(CONFIG_RUST) += luna_boot.o' "$MAKEFILE" ||
    printf '%s\n' 'obj-$(CONFIG_RUST) += luna_boot.o' >> "$MAKEFILE"
grep -Fq 'obj-$(CONFIG_RUST) += luna_exec.o' "$MAKEFILE" ||
    printf '%s\n' 'obj-$(CONFIG_RUST) += luna_exec.o' >> "$MAKEFILE"

python3 - "$SETUP_C" "$INIT_C" "$EXEC_C" "$BINFMT_H" <<'PY'
from pathlib import Path
import sys

setup_c, init_c, exec_c, binfmt_h = map(Path, sys.argv[1:])

def once(path: Path, old: str, new: str, marker: str) -> None:
    s = path.read_text()
    if marker in s:
        return
    if old not in s:
        raise SystemExit(f"overlay insertion point not found in {path}: {old!r}")
    path.write_text(s.replace(old, new, 1))

once(
    setup_c,
    "parse_setup_data();\n",
    """parse_setup_data();
#ifdef CONFIG_RUST
\textern void x86_luna_boot_parse(u64 setup_data_phys);
\tx86_luna_boot_parse(boot_params.hdr.setup_data);
#endif
""",
    "x86_luna_boot_parse(boot_params.hdr.setup_data);",
)

once(
    init_c,
    "#include <linux/binfmts.h>\n",
    """#include <linux/binfmts.h>
#ifdef CONFIG_RUST
extern int x86_luna_exec_init(void);
extern bool x86_luna_boot_available(void);
static bool luna_init_started;
#endif
""",
    "extern int x86_luna_exec_init(void);",
)

once(
    init_c,
    """\twait_for_initramfs();
\tconsole_on_rootfs();
""",
    """\twait_for_initramfs();
#ifdef CONFIG_RUST
\t/* Direct-init mode needs devtmpfs before console_on_rootfs(). */
\tif (x86_luna_boot_available())
\t\tdevtmpfs_mount();
#endif
\tconsole_on_rootfs();
#ifdef CONFIG_RUST
\tif (x86_luna_boot_available()) {
\t\tint luna_ret = x86_luna_exec_init();
\t\tif (!luna_ret) {
\t\t\tluna_init_started = true;
\t\t\treturn;
\t\t}
\t\tpanic("Luna: direct init execution failed (error %d).", luna_ret);
\t}
#endif
""",
    "Luna: direct init execution failed",
)

once(
    init_c,
    """\tdo_sysctl_args();

\tif (ramdisk_execute_command) {
""",
    """\tdo_sysctl_args();

#ifdef CONFIG_RUST
\tif (luna_init_started)
\t\treturn 0;
#endif

\tif (ramdisk_execute_command) {
""",
    "if (luna_init_started)",
)

exec_marker = "int kernel_execve_file(struct file *file,"
exec_snippet = r'''/* Project Luna: execute a kernel-created memory-backed executable object. */
int kernel_execve_file(struct file *file,
		       struct file *handoff,
		       const char *const *argv,
		       const char *const *envp)
{
	int fd, retval;

	if (!file || !handoff)
		return -EINVAL;

	fd = get_unused_fd_flags(0);
	if (fd < 0)
		return fd;
	if (fd != 3) {
		put_unused_fd(fd);
		return -EBUSY;
	}

	get_file(handoff);
	fd_install(fd, handoff);

	inode_lock(file_inode(file));
	file_inode(file)->i_mode = (file_inode(file)->i_mode & S_IFMT) | 0700;
	inode_unlock(file_inode(file));

	fd = get_unused_fd_flags(0);
	if (fd < 0) {
		close_fd(3);
		return fd;
	}

	get_file(file);
	fd_install(fd, file);

	{
		CLASS(filename_kernel, filename)("");
		CLASS(bprm, bprm)(fd, filename, AT_EMPTY_PATH);

		if (IS_ERR(bprm)) {
			retval = PTR_ERR(bprm);
			close_fd(fd);
			close_fd(3);
			return retval;
		}

		retval = count_strings_kernel(argv);
		if (retval <= 0) {
			close_fd(fd);
			close_fd(3);
			return retval < 0 ? retval : -EINVAL;
		}
		bprm->argc = retval;

		retval = count_strings_kernel(envp);
		if (retval < 0) {
			close_fd(fd);
			close_fd(3);
			return retval;
		}
		bprm->envc = retval;

		retval = bprm_stack_limits(bprm);
		if (retval < 0) {
			close_fd(fd);
			close_fd(3);
			return retval;
		}

		retval = copy_string_kernel(bprm->filename, bprm);
		if (retval < 0) {
			close_fd(fd);
			close_fd(3);
			return retval;
		}
		bprm->exec = bprm->p;

		retval = copy_strings_kernel(bprm->envc, envp, bprm);
		if (retval < 0) {
			close_fd(fd);
			close_fd(3);
			return retval;
		}
		retval = copy_strings_kernel(bprm->argc, argv, bprm);
		if (retval < 0) {
			close_fd(fd);
			close_fd(3);
			return retval;
		}

		retval = bprm_execve(bprm);
	}

	close_fd(fd);
	if (retval)
		close_fd(3);
	return retval;
}
EXPORT_SYMBOL_GPL(kernel_execve_file);

'''
def insert_before(path: Path, marker: str, snippet: str) -> None:
    s = path.read_text()
    if marker in s:
        return
    pivot = "void set_binfmt(struct linux_binfmt *new)"
    if pivot not in s:
        raise SystemExit(f"binfmt insertion point not found in {path}")
    path.write_text(s.replace(pivot, snippet + pivot, 1))

insert_before(exec_c, exec_marker, exec_snippet)

once(
    binfmt_h,
    "int kernel_execve(const char *filename,\n",
    "int kernel_execve_file(struct file *file, struct file *handoff,\n"
    "\t\t\t       const char *const *argv,\n"
    "\t\t\t       const char *const *envp);\n\n"
    "int kernel_execve(const char *filename,\n",
    "int kernel_execve_file(struct file *file",
)
PY

echo "Applied Luna direct-init kernel overlay: $SRC"

# Alpine foundation experiment: attach the selected real root filesystem
# directly over the initial namespace root and skip rootfs pivoting.
python3 "${REPO_ROOT}/tools/apply-luna-direct-root-overlay.py" "$SRC"
