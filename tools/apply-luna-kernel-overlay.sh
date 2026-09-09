#!/usr/bin/env bash
set -euo pipefail

SRC="$1"
REPO_ROOT="$2"

RUST_SRC="${REPO_ROOT}/kernel/rust/luna_boot.rs"
EXEC_SRC="${REPO_ROOT}/kernel/rust/luna_exec.rs"
RUST_DST="${SRC}/arch/x86/kernel/luna_boot.rs"
EXEC_DST="${SRC}/arch/x86/kernel/luna_exec.rs"
MAKEFILE="${SRC}/arch/x86/kernel/Makefile"
SETUP_C="${SRC}/arch/x86/kernel/setup.c"
INIT_C="${SRC}/init/main.c"
EXEC_C="${SRC}/fs/exec.c"

[ -f "$RUST_SRC" ] || { echo "missing Luna Rust source: $RUST_SRC" >&2; exit 1; }
[ -f "$EXEC_SRC" ] || { echo "missing Luna Rust launcher: $EXEC_SRC" >&2; exit 1; }
[ -f "$MAKEFILE" ] || { echo "missing Linux x86 kernel Makefile: $MAKEFILE" >&2; exit 1; }
[ -f "$SETUP_C" ] || { echo "missing Linux x86 setup.c: $SETUP_C" >&2; exit 1; }
[ -f "$INIT_C" ] || { echo "missing Linux init/main.c: $INIT_C" >&2; exit 1; }
[ -f "$EXEC_C" ] || { echo "missing Linux fs/exec.c: $EXEC_C" >&2; exit 1; }

for pair in "$RUST_SRC:$RUST_DST" "$EXEC_SRC:$EXEC_DST"; do
    SRC_FILE="${pair%%:*}"
    DST_FILE="${pair#*:}"
    if [ -e "$DST_FILE" ]; then
        cmp -s "$SRC_FILE" "$DST_FILE" || {
            echo "refusing to overwrite existing different $DST_FILE" >&2
            exit 1
        }
    else
        cp "$SRC_FILE" "$DST_FILE"
    fi
done

python3 - "$MAKEFILE" "$SETUP_C" "$INIT_C" "$EXEC_C" <<'PY'
from pathlib import Path
import sys

makefile = Path(sys.argv[1])
setup = Path(sys.argv[2])
init = Path(sys.argv[3])
exec_c = Path(sys.argv[4])

text = makefile.read_text()
for line in (
    "obj-$(CONFIG_RUST) += luna_boot.o\n",
    "obj-$(CONFIG_RUST) += luna_exec.o\n",
):
    if line not in text:
        if not text.endswith("\n"):
            text += "\n"
        text += line
makefile.write_text(text)

text = setup.read_text()
bridge = '''\n#ifdef CONFIG_RUST\nextern void x86_luna_boot_parse(u64 setup_data_phys);\nx86_luna_boot_parse(boot_params.hdr.setup_data);\n#else\n#error "Project Luna requires CONFIG_RUST"\n#endif\n'''
if "x86_luna_boot_parse(boot_params.hdr.setup_data);" not in text:
    anchor = "\tparse_setup_data();\n"
    if anchor not in text:
        raise SystemExit(f"cannot locate setup_data parser in {setup}")
    text = text.replace(anchor, anchor + bridge, 1)
setup.write_text(text)

text = init.read_text()
launcher = '''\n#ifdef CONFIG_RUST\nextern int x86_luna_exec_init(void);\n#endif\n'''
if "extern int x86_luna_exec_init(void);" not in text:
    marker = "#include <linux/binfmts.h>\n"
    if marker not in text:
        raise SystemExit(f"cannot locate binfmts include in {init}")
    text = text.replace(marker, marker + launcher, 1)

call = '''\n\tif (IS_ENABLED(CONFIG_RUST)) {\n\t\tint luna_ret = x86_luna_exec_init();\n\t\tif (luna_ret)\n\t\t\tpanic("Luna: direct luna-init execution failed (error %d).", luna_ret);\n\t}\n'''
if "x86_luna_exec_init();" not in text:
    anchor = "\tconsole_on_rootfs();\n"
    if anchor not in text:
        raise SystemExit(f"cannot locate console_on_rootfs() in {init}")
    text = text.replace(anchor, anchor + call, 1)
init.write_text(text)

# Keep the Luna-specific execution policy in Rust while adding only the
# smallest generic bridge needed to feed a kernel-created anonymous file into
# Linux's existing kernel_execve/binfmt machinery. No Luna storage, rootfs, or
# bootstrap policy belongs in this C hook.
text = exec_c.read_text()
if "int kernel_execve_file(struct file *file," not in text:
    marker = "void set_binfmt(struct linux_binfmt *new)\n"
    if marker not in text:
        raise SystemExit(f"cannot locate set_binfmt() in {exec_c}")

    bridge = '''\n/*\n * Luna-only kernel-internal adapter. The caller supplies an already-created\n * memory-backed executable object; this helper only feeds it through the\n * existing kernel exec path so Linux keeps ownership of ELF loading, VM\n * construction, credentials and process setup. The object is never opened by\n * pathname.\n */\nint kernel_execve_file(struct file *file,\n\t\t\t       const char *const *argv,\n\t\t\t       const char *const *envp)\n{\n\tint fd, retval;\n\n\tif (!file)\n\t\treturn -EINVAL;\n\n\t/* The anonymous shmem object is kernel-created and trusted by Luna.\n\t * Give the VFS execute permission required by do_open_execat(). */\n\tinode_lock(file_inode(file));\n\tfile_inode(file)->i_mode = (file_inode(file)->i_mode & S_IFMT) | 0700;\n\tinode_unlock(file_inode(file));\n\n\tfd = get_unused_fd_flags(O_CLOEXEC);\n\tif (fd < 0)\n\t\treturn fd;\n\n\tget_file(file);\n\tfd_install(fd, file);\n\n\t{\n\t\tCLASS(filename_kernel, filename)("");\n\t\tCLASS(bprm, bprm)(fd, filename, AT_EMPTY_PATH);\n\t\n\t\tif (IS_ERR(bprm)) {\n\t\t\tretval = PTR_ERR(bprm);\n\t\t\tclose_fd(fd);\n\t\t\treturn retval;\n\t\t}\n\n\t\tretval = count_strings_kernel(argv);\n\t\tif (WARN_ON_ONCE(retval == 0)) {\n\t\t\tclose_fd(fd);\n\t\t\treturn -EINVAL;\n\t\t}\n\t\tif (retval < 0) {\n\t\t\tclose_fd(fd);\n\t\t\treturn retval;\n\t\t}\n\t\tbprm->argc = retval;\n\n\t\tretval = count_strings_kernel(envp);\n\t\tif (retval < 0) {\n\t\t\tclose_fd(fd);\n\t\t\treturn retval;\n\t\t}\n\t\tbprm->envc = retval;\n\n\t\tretval = bprm_stack_limits(bprm);\n\t\tif (retval < 0) {\n\t\t\tclose_fd(fd);\n\t\t\treturn retval;\n\t\t}\n\n\t\tretval = copy_string_kernel(bprm->filename, bprm);\n\t\tif (retval < 0) {\n\t\t\tclose_fd(fd);\n\t\t\treturn retval;\n\t\t}\n\t\tbprm->exec = bprm->p;\n\n\t\tretval = copy_strings_kernel(bprm->envc, envp, bprm);\n\t\tif (retval < 0) {\n\t\t\tclose_fd(fd);\n\t\t\treturn retval;\n\t\t}\n\n\t\tretval = copy_strings_kernel(bprm->argc, argv, bprm);\n\t\tif (retval < 0) {\n\t\t\tclose_fd(fd);\n\t\t\treturn retval;\n\t\t}\n\n\t\tretval = bprm_execve(bprm);\n\t}\n\n\tclose_fd(fd);\n\treturn retval;\n}\nEXPORT_SYMBOL_GPL(kernel_execve_file);\n\n'''
    text = text.replace(marker, bridge + marker, 1)
    exec_c.write_text(text)
PY

echo "Applied Luna kernel overlay to: $SRC"
