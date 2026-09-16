#!/usr/bin/env python3
"""Built-in verification checks for Project Luna autonomous tasks."""
from __future__ import annotations

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def command(argv: list[str], timeout: int = 900) -> tuple[int, str]:
    try:
        proc = subprocess.run(argv, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, timeout=timeout)
    except subprocess.TimeoutExpired as exc:
        output = exc.stdout or ""
        return 124, f"timeout after {timeout}s\n{output}"
    return proc.returncode, proc.stdout


def run_check(name: str) -> tuple[bool, str]:
    checks: dict[str, tuple[list[str], int]] = {
        "git-diff-check": (["git", "diff", "--check"], 120),
        "luna-init-test": (["cargo", "test", "--manifest-path",
                             "components/system/luna-init/Cargo.toml"], 900),
        "luna-init-static": (["bash", "tools/check-luna-init-static.sh"], 120),
        "luna-static": (["bash", "tools/check-luna-static.sh"], 120),
        "boot-ovmf": (["bash", "tools/test-luna-boot-ovmf.sh"], 1200),
    }
    if name not in checks:
        return False, f"unknown verification check: {name}"
    rc, output = command(*checks[name])
    return rc == 0, output


def main(names: list[str]) -> int:
    failed = False
    for name in names:
        ok, output = run_check(name)
        print(f"=== {name}: {'PASS' if ok else 'FAIL'} ===")
        print(output.rstrip())
        if not ok:
            failed = True
    return 1 if failed else 0


if __name__ == "__main__":
    import sys
    raise SystemExit(main(sys.argv[1:]))
