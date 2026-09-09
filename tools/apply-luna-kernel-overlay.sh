#!/usr/bin/env bash
set -euo pipefail

SRC="$1"
REPO_ROOT="$2"

RUST_SRC="${REPO_ROOT}/kernel/rust/luna_boot.rs"
RUST_DST="${SRC}/arch/x86/kernel/luna_boot.rs"
MAKEFILE="${SRC}/arch/x86/kernel/Makefile"
SETUP_C="${SRC}/arch/x86/kernel/setup.c"

[ -f "$RUST_SRC" ] || { echo "missing Luna Rust source: $RUST_SRC" >&2; exit 1; }
[ -f "$MAKEFILE" ] || { echo "missing Linux x86 kernel Makefile: $MAKEFILE" >&2; exit 1; }
[ -f "$SETUP_C" ] || { echo "missing Linux x86 setup.c: $SETUP_C" >&2; exit 1; }

if [ -e "$RUST_DST" ]; then
    cmp -s "$RUST_SRC" "$RUST_DST" || {
        echo "refusing to overwrite existing different $RUST_DST" >&2
        exit 1
    }
else
    cp "$RUST_SRC" "$RUST_DST"
fi

python3 - "$MAKEFILE" "$SETUP_C" <<'PY'
from pathlib import Path
import re
import sys

makefile = Path(sys.argv[1])
setup = Path(sys.argv[2])

text = makefile.read_text()
line = "obj-$(CONFIG_RUST) += luna_boot.o\n"
if line not in text:
    # Linux's x86 kernel Makefile has changed ordering over time. Insert
    # immediately after the first built-in object assignment instead of
    # relying on one particular object name.
    match = re.search(r"(?m)^[ \t]*obj-y[ \t]*(?::=|\+=|=)", text)
    if not match:
        raise SystemExit(f"cannot locate obj-y Kbuild assignment in {makefile}")
    line_end = text.find("\n", match.start())
    if line_end < 0:
        raise SystemExit(f"cannot locate end of obj-y assignment in {makefile}")
    insertion = line_end + 1
    text = text[:insertion] + line + text[insertion:]
    makefile.write_text(text)

text = setup.read_text()
bridge = '''\n#ifdef CONFIG_RUST\nextern void x86_luna_boot_parse(u64 setup_data_phys);\nx86_luna_boot_parse(boot_params.hdr.setup_data);\n#else\n#error "Project Luna requires CONFIG_RUST"\n#endif\n'''
anchor = "\tparse_setup_data();\n"
if "x86_luna_boot_parse(boot_params.hdr.setup_data);" not in text:
    if anchor not in text:
        raise SystemExit(f"cannot locate setup_data parser in {setup}")
    text = text.replace(anchor, anchor + bridge, 1)
    setup.write_text(text)
PY

echo "Applied Luna kernel overlay to: $SRC"
