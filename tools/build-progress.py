#!/usr/bin/env python3
"""Run a build command with a compact progress display and persistent log.

Normal build output is kept in the log instead of flooding the terminal.
Warnings/errors are surfaced live and summarized again at the end.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import time
from pathlib import Path

try:
    from tqdm import tqdm
except ImportError:
    tqdm = None


DEFAULT_ACTION_RE = re.compile(
    r"^\s+(?:HOST)?(?:CC|CXX|RUSTC|AR|LD|AS|OBJCOPY|OBJDUMP|STRIP|GEN|BUILD|BINDGEN|MODPOST|ZOFFSET)"
)
PROBLEM_RE = re.compile(
    r"(?:\berror\b|\bwarning\b|\bfatal\b|undefined reference|section mismatch|panic(?:ked)?|FAILED|FAILURE)",
    re.IGNORECASE,
)


def format_seconds(value: float | None) -> str:
    if value is None or value < 0:
        return "--:--"
    seconds = int(value)
    days, seconds = divmod(seconds, 86400)
    hours, seconds = divmod(seconds, 3600)
    minutes, seconds = divmod(seconds, 60)
    if days:
        return f"{days}d {hours:02d}:{minutes:02d}:{seconds:02d}"
    if hours:
        return f"{hours:02d}:{minutes:02d}:{seconds:02d}"
    return f"{minutes:02d}:{seconds:02d}"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--label", required=True, help="Progress-bar label")
    parser.add_argument("--log", required=True, type=Path, help="Full build log path")
    parser.add_argument("--total", type=int, default=0, help="Approximate number of work units")
    parser.add_argument(
        "--action-regex",
        default=DEFAULT_ACTION_RE.pattern,
        help="Regex matching lines that count as completed work units",
    )
    parser.add_argument("command", nargs=argparse.REMAINDER, help="Command to execute after --")
    return parser


def print_problem(line: str) -> None:
    text = line.rstrip()
    if not text:
        return
    if tqdm is not None:
        tqdm.write(f"\033[1;31m[BUILD PROBLEM]\033[0m {text}")
    else:
        print(f"[BUILD PROBLEM] {text}", flush=True)


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if not args.command or args.command[0] != "--":
        parser.error("command must be introduced with --")

    command = args.command[1:]
    args.log.parent.mkdir(parents=True, exist_ok=True)
    action_re = re.compile(args.action_regex)
    problems: list[str] = []
    start = time.monotonic()
    completed = 0

    with args.log.open("w", encoding="utf-8", buffering=1) as log:
        log.write(f"# Project Luna build log\n# started={time.strftime('%Y-%m-%d %H:%M:%S %z')}\n")
        log.write("# command=" + " ".join(command) + "\n\n")

        process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
            bufsize=1,
        )

        if tqdm is not None:
            bar = tqdm(
                total=args.total or None,
                desc=args.label,
                unit="step",
                dynamic_ncols=True,
                bar_format=(
                    "{l_bar}{bar}| {n_fmt} "
                    "[{elapsed}<{remaining}, {rate_fmt}]"
                    if args.total
                    else "{desc}: {n_fmt} steps [{elapsed}, {rate_fmt}]"
                ),
            )
        else:
            bar = None
            print(f"{args.label}: build started", flush=True)

        assert process.stdout is not None
        try:
            for raw_line in process.stdout:
                line = raw_line.rstrip("\n")
                log.write(raw_line)

                if action_re.search(line):
                    completed += 1
                    if bar is not None:
                        bar.update(1)

                if PROBLEM_RE.search(line):
                    problems.append(line)
                    print_problem(line)
        finally:
            process.stdout.close()
            return_code = process.wait()
            if bar is not None:
                if args.total and completed < args.total:
                    bar.total = completed
                bar.close()

    elapsed = time.monotonic() - start
    print()
    print(f"{args.label}: {'успешно' if return_code == 0 else 'ОШИБКА'}")
    print(f"Время: {format_seconds(elapsed)}")
    print(f"Шагов: {completed}")
    print(f"Лог:   {args.log}")

    if problems:
        print(f"\nПроблемы ({len(problems)} строк):")
        seen: set[str] = set()
        for line in problems:
            if line not in seen:
                seen.add(line)
                print(f"  {line}")

    return return_code


if __name__ == "__main__":
    raise SystemExit(main())
