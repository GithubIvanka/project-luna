#!/usr/bin/env python3
"""Minimal sequential Project Luna agent orchestrator."""
from __future__ import annotations

import argparse
import datetime as dt
import os
import pathlib
import subprocess
import sys
import time
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
TASKS_FILE = HERE / "tasks.toml"
STATE_FILE = HERE / "state.toml"
LOG_DIR = HERE / "logs"
DSH = pathlib.Path.home() / ".npm/_npx/1e7f6d9597241db0/node_modules/.bin/dsh"
OPENCODE = pathlib.Path.home() / ".local/bin/opencode"
BRANCH = "alpha-development"


def run(cmd: list[str], *, timeout: int | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, timeout=timeout)


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def load_tasks() -> dict:
    with TASKS_FILE.open("rb") as fh:
        return tomllib.load(fh)


def git_status() -> list[str]:
    return run(["git", "status", "--short"]).stdout.splitlines()


def require_branch() -> None:
    current = run(["git", "branch", "--show-current"]).stdout.strip()
    if current != BRANCH:
        raise RuntimeError(f"Refusing to run: expected {BRANCH}, got {current or '<detached>'}")


def save_state(task: str, phase: str, attempt: int, agent: str, result: str) -> None:
    content = f'''version = 1\ncurrent_task = {task!r}\nphase = {phase!r}\nattempt = {attempt}\nlast_agent = {agent!r}\nlast_result = {result!r}\nupdated_at = {now()!r}\n\n[limits]\nmax_attempts_per_task = 3\nharness_timeout_seconds = 3600\nopencode_timeout_seconds = 1800\n\n[policy]\nprotected_branch = "develop"\nworking_branch = "alpha-development"\nallow_host_reboot = false\nallow_physical_disk_changes = false\nallow_user_data_deletion = false\n'''
    STATE_FILE.write_text(content)


def log_path(task: str, agent: str) -> pathlib.Path:
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    return LOG_DIR / f"{stamp}-{task}-{agent}.log"


def append_log(path: pathlib.Path, text: str) -> None:
    path.open("a", encoding="utf-8").write(text)


def task_ready(task: dict, completed: set[str]) -> bool:
    return all(dep in completed for dep in task.get("depends_on", []))


def pick_task(tasks: list[dict]) -> dict | None:
    completed = {t["id"] for t in tasks if t.get("status") == "done"}
    pending = [t for t in tasks if t.get("status") == "pending" and task_ready(t, completed)]
    pending.sort(key=lambda t: (0 if t.get("priority") == "P0" else 1, t["id"]))
    return pending[0] if pending else None


def ensure_pristine_for_agent_start(baseline: list[str]) -> None:
    current = git_status()
    if current != baseline:
        raise RuntimeError("Working tree changed outside Luna Agent; refusing to start another agent task.")


def run_agent(agent: str, prompt: str, timeout: int, log: pathlib.Path) -> int:
    if agent == "harness":
        cmd = [str(DSH), "--profile", "headless", prompt]
    elif agent == "opencode-review":
        cmd = [str(OPENCODE), "run", "--auto", "--model", "openrouter/deepseek/deepseek-v4-flash-latest", prompt]
    else:
        raise RuntimeError(f"Unknown agent: {agent}")
    append_log(log, f"\n=== {now()} COMMAND ===\n{' '.join(cmd)}\n")
    proc = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, timeout=timeout)
    append_log(log, f"\n=== STDOUT ===\n{proc.stdout}\n=== STDERR ===\n{proc.stderr}\n=== EXIT {proc.returncode} ===\n")
    return proc.returncode


def verify_after_agent() -> None:
    result = run(["git", "diff", "--check"])
    if result.returncode != 0:
        raise RuntimeError("git diff --check failed after agent run")


def prompt_for(task: dict, kind: str) -> str:
    base = (HERE / "prompts" / ("implement.md" if kind == "harness" else "review.md")).read_text()
    return base + f"\n\nCURRENT TASK\nID: {task['id']}\nTITLE: {task['title']}\nPRIORITY: {task['priority']}\n"


def mark_task(tasks: list[dict], task_id: str, status: str) -> None:
    for task in tasks:
        if task["id"] == task_id:
            task["status"] = status
            return


def write_tasks(data: dict, tasks: list[dict]) -> None:
    lines = ["[queue]", "version = 1", 'branch = "alpha-development"', ""]
    for task in tasks:
        lines += ["[[tasks]]", f'id = "{task["id"]}"', f'priority = "{task["priority"]}"',
                  f'agent = "{task["agent"]}"', f'status = "{task["status"]}"',
                  f'title = "{task["title"]}"']
        if task.get("depends_on"):
            deps = ", ".join(f'"{x}"' for x in task["depends_on"])
            lines.append(f"depends_on = [{deps}]")
        lines.append("")
    TASKS_FILE.write_text("\n".join(lines), encoding="utf-8")


def run_once() -> bool:
    require_branch()
    data = load_tasks()
    tasks = data["tasks"]
    task = pick_task(tasks)
    if task is None:
        print("No ready pending tasks.")
        return False

    baseline = git_status()
    baseline_head = run(["git", "rev-parse", "HEAD"]).stdout.strip()
    if baseline:
        raise RuntimeError("Working tree must be clean before an autonomous task. Create a WIP checkpoint first.")
    attempt = 1
    save_state(task["id"], "implementation", attempt, task["agent"], "starting")
    log = log_path(task["id"], task["agent"])
    kind = task["agent"]
    timeout = 3600 if kind == "harness" else 1800
    rc = run_agent(kind, prompt_for(task, kind), timeout, log)

    if rc != 0:
        save_state(task["id"], "failed", attempt, kind, f"exit_{rc}")
        mark_task(tasks, task["id"], "pending")
        write_tasks(data, tasks)
        return True

    current = git_status()
    if current:
        save_state(task["id"], "failed", attempt, kind, "dirty_tree_after_agent")
        print("Agent exited successfully but left uncommitted changes; refusing to mark task done.")
        return True
    if run(["git", "rev-parse", "HEAD"]).stdout.strip() == baseline_head:
        save_state(task["id"], "failed", attempt, kind, "no_commit")
        print("Agent exited successfully but created no commit; refusing to advance the queue.")
        return True
    verify_after_agent()
    save_state(task["id"], "completed", attempt, kind, "committed_and_clean")
    mark_task(tasks, task["id"], "done")
    write_tasks(data, tasks)
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description="Sequential Luna Agent orchestrator")
    parser.add_argument("--once", action="store_true", help="run one ready task and exit")
    parser.add_argument("--continuous", action="store_true", help="keep processing ready tasks")
    args = parser.parse_args()
    if not args.once and not args.continuous:
        parser.error("choose --once or --continuous")

    if args.once:
        return 0 if run_once() is not None else 1

    while True:
        progressed = run_once()
        if not progressed:
            time.sleep(30)
        else:
            time.sleep(5)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("Luna Agent stopped by user.")
        raise SystemExit(130)
    except Exception as exc:
        print(f"Luna Agent fatal error: {exc}", file=sys.stderr)
        raise SystemExit(2)
