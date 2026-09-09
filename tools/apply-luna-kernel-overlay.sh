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

[ -f "$RUST_SRC" ] || { echo "missing Luna Rust source: $RUST_SRC" >&2; exit 1; }
[ -f "$EXEC_SRC" ] || { echo "missing Luna Rust launcher: $EXEC_SRC" >&2; exit 1; }
[ -f "$MAKEFILE" ] || { echo "missing Linux x86 kernel Makefile: $MAKEFILE" >&2; exit 1; }
[ -f "$SETUP_C" ] || { echo "missing Linux x86 setup.c: $SETUP_C" >&2; exit 1; }
[ -f "$INIT_C" ] || { echo "missing Linux init/main.c: $INIT_C" >&2; exit 1; }

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

python3 - "$MAKEFILE" "$SETUP_C" "$INIT_C" <<'PY'
from pathlib import Path
import sys

makefile = Path(sys.argv[1])
setup = Path(sys.argv[2])
init = Path(sys.argv[3])

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
PY

echo "Applied Luna kernel overlay to: $SRC"
