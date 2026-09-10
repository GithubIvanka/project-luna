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
BINFMT_H="${SRC}/include/linux/binfmts.h"

[ -f "$RUST_SRC" ] || { echo "missing Luna Rust source: $RUST_SRC" >&2; exit 1; }
[ -f "$EXEC_SRC" ] || { echo "missing Luna Rust launcher: $EXEC_SRC" >&2; exit 1; }
[ -f "$MAKEFILE" ] || { echo "missing Linux x86 kernel Makefile: $MAKEFILE" >&2; exit 1; }
[ -f "$SETUP_C" ] || { echo "missing Linux x86 setup.c: $SETUP_C" >&2; exit 1; }
[ -f "$INIT_C" ] || { echo "missing Linux init/main.c: $INIT_C" >&2; exit 1; }
[ -f "$EXEC_C" ] || { echo "missing Linux fs/exec.c: $EXEC_C" >&2; exit 1; }
[ -f "$BINFMT_H" ] || { echo "missing Linux include/linux/binfmts.h: $BINFMT_H" >&2; exit 1; }

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

if ! grep -Fq 'obj-$(CONFIG_RUST) += luna_boot.o' "$MAKEFILE"; then
    printf '%s\n' 'obj-$(CONFIG_RUST) += luna_boot.o' >> "$MAKEFILE"
fi
if ! grep -Fq 'obj-$(CONFIG_RUST) += luna_exec.o' "$MAKEFILE"; then
    printf '%s\n' 'obj-$(CONFIG_RUST) += luna_exec.o' >> "$MAKEFILE"
fi

if ! grep -Fq 'x86_luna_boot_parse(boot_params.hdr.setup_data);' "$SETUP_C"; then
    setup_snippet="$(mktemp)"
    trap 'rm -f "$setup_snippet" "$init_include_snippet" "$init_call_snippet" "$exec_bridge" 2>/dev/null || true' EXIT
    cat > "$setup_snippet" <<'EOF'
#ifdef CONFIG_RUST
extern void x86_luna_boot_parse(u64 setup_data_phys);
x86_luna_boot_parse(boot_params.hdr.setup_data);
#else
#error "Project Luna requires CONFIG_RUST"
#endif
EOF
    awk -v snippet="$setup_snippet" '
        /^[[:space:]]*parse_setup_data\(\);[[:space:]]*$/ {
            while ((getline line < snippet) > 0) print line
            close(snippet)
        }
        { print }
    ' "$SETUP_C" > "$SETUP_C.tmp"
    mv "$SETUP_C.tmp" "$SETUP_C"
fi

if ! grep -Fq 'extern int x86_luna_exec_init(void);' "$INIT_C"; then
    init_include_snippet="$(mktemp)"
    cat > "$init_include_snippet" <<'EOF'
#ifdef CONFIG_RUST
extern int x86_luna_exec_init(void);
#endif
EOF
    awk -v snippet="$init_include_snippet" '
        /#include <linux\/binfmts\.h>/ {
            print
            while ((getline line < snippet) > 0) print line
            close(snippet)
            next
        }
        { print }
    ' "$INIT_C" > "$INIT_C.tmp"
    mv "$INIT_C.tmp" "$INIT_C"
fi

if ! grep -Fq 'x86_luna_exec_init();' "$INIT_C"; then
    init_call_snippet="$(mktemp)"
    cat > "$init_call_snippet" <<'EOF'

#ifdef CONFIG_RUST
	if (IS_ENABLED(CONFIG_RUST)) {
		int luna_ret = x86_luna_exec_init();
		if (luna_ret)
			panic("Luna: direct luna-init execution failed (error %d).", luna_ret);
	}
#endif
EOF
    awk -v snippet="$init_call_snippet" '
        /^[[:space:]]*console_on_rootfs\(\);[[:space:]]*$/ {
            print
            while ((getline line < snippet) > 0) print line
            close(snippet)
            next
        }
        { print }
    ' "$INIT_C" > "$INIT_C.tmp"
    mv "$INIT_C.tmp" "$INIT_C"
fi

# Keep the Luna-specific execution policy in Rust while adding only the
# smallest generic bridge needed to feed a kernel-created anonymous file into
# Linux's existing kernel_execve/binfmt machinery. No Luna storage, rootfs, or
# bootstrap policy belongs in this C hook.
if ! grep -Fq 'int kernel_execve_file(struct file *file,' "$EXEC_C"; then
    exec_bridge="$(mktemp)"
    cat > "$exec_bridge" <<'EOF'

/*
 * Luna-only kernel-internal adapter. The caller supplies an already-created
 * memory-backed executable object; this helper only feeds it through the
 * existing kernel exec path so Linux keeps ownership of ELF loading, VM
 * construction, credentials and process setup. The object is never opened by
 * pathname.
 */
int kernel_execve_file(struct file *file,
			       const char *const *argv,
			       const char *const *envp)
{
	int fd, retval;

	if (!file)
		return -EINVAL;

	/* The anonymous shmem object is kernel-created and trusted by Luna.
	 * Give the VFS execute permission required by do_open_execat(). */
	inode_lock(file_inode(file));
	file_inode(file)->i_mode = (file_inode(file)->i_mode & S_IFMT) | 0700;
	inode_unlock(file_inode(file));

	fd = get_unused_fd_flags(O_CLOEXEC);
	if (fd < 0)
		return fd;

	get_file(file);
	fd_install(fd, file);

	{
		CLASS(filename_kernel, filename)("");
		CLASS(bprm, bprm)(fd, filename, AT_EMPTY_PATH);

		if (IS_ERR(bprm)) {
			retval = PTR_ERR(bprm);
			close_fd(fd);
			return retval;
		}

		retval = count_strings_kernel(argv);
		if (WARN_ON_ONCE(retval == 0)) {
			close_fd(fd);
			return -EINVAL;
		}
		if (retval < 0) {
			close_fd(fd);
			return retval;
		}
		bprm->argc = retval;

		retval = count_strings_kernel(envp);
		if (retval < 0) {
			close_fd(fd);
			return retval;
		}
		bprm->envc = retval;

		retval = bprm_stack_limits(bprm);
		if (retval < 0) {
			close_fd(fd);
			return retval;
		}

		retval = copy_string_kernel(bprm->filename, bprm);
		if (retval < 0) {
			close_fd(fd);
			return retval;
		}
		bprm->exec = bprm->p;

		retval = copy_strings_kernel(bprm->envc, envp, bprm);
		if (retval < 0) {
			close_fd(fd);
			return retval;
		}

		retval = copy_strings_kernel(bprm->argc, argv, bprm);
		if (retval < 0) {
			close_fd(fd);
			return retval;
		}

		retval = bprm_execve(bprm);
	}

	close_fd(fd);
	return retval;
}
EXPORT_SYMBOL_GPL(kernel_execve_file);

EOF
    awk -v snippet="$exec_bridge" '
        /^void set_binfmt\(struct linux_binfmt \*new\)/ {
            while ((getline line < snippet) > 0) print line
            close(snippet)
        }
        { print }
    ' "$EXEC_C" > "$EXEC_C.tmp"
    mv "$EXEC_C.tmp" "$EXEC_C"
fi

if ! grep -Fq 'int kernel_execve_file(struct file *file,' "$BINFMT_H"; then
    cat >> "$BINFMT_H" <<'EOF'

/* Project Luna: execute a kernel-created memory-backed file. */
int kernel_execve_file(struct file *file,
			       const char *const *argv,
			       const char *const *envp);
EOF
fi

echo "Applied Luna kernel overlay to: $SRC"
