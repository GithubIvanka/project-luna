#!/usr/bin/env python3
"""Sequential Project Luna agent orchestrator.

The runner coordinates headless Harness and non-interactive OpenCode in one
working tree. It never cleans or resets the repository and pauses when the
working tree is changed outside the runner/agent transaction.
"""
from __future__ import annotations

import argparse
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time
import tomllib

from verification import VERIFICATION_CHECKS, run_check

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
TASKS_FILE = HERE / "tasks.toml"
LOG_DIR = HERE / "logs"
STATE_DIR = Path.home() / ".local/state/project-luna/luna-agent"
STATE_FILE = STATE_DIR / "state.json"
LOCK_FILE = STATE_DIR / "runner.lock"
HEARTBEAT_FILE = STATE_DIR / "heartbeat.json"
PAUSE_FILE = STATE_DIR / "PAUSE"
BRANCH = "alpha-development"
MAX_ATTEMPTS = int(os.environ.get("LUNA_AGENT_MAX_ATTEMPTS", "3"))
MAX_TURNS_PER_ATTEMPT = int(os.environ.get("LUNA_AGENT_MAX_TURNS", "6"))
HARNESS_TIMEOUT = int(os.environ.get("LUNA_AGENT_HARNESS_TIMEOUT", "3600"))
OPENCODE_TIMEOUT = int(os.environ.get("LUNA_AGENT_OPENCODE_TIMEOUT", "1800"))
OPENCODE_MODEL = os.environ.get("LUNA_AGENT_OPENCODE_MODEL", "openrouter/deepseek/deepseek-v4-flash-latest")
OPENCODE_CONFIG = os.environ.get("LUNA_AGENT_OPENCODE_CONFIG")
OFFLINE_MODE = os.environ.get("LUNA_AGENT_OFFLINE", "0") == "1"
TASKS_VERSION = 2


def run(cmd: list[str], timeout: int | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, timeout=timeout)


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def load_tasks() -> list[dict]:
    with TASKS_FILE.open("rb") as fh:
        data = tomllib.load(fh)
    queue = data.get("queue", {})
    if queue.get("version") != TASKS_VERSION:
        raise RuntimeError(f"unsupported task queue version: {queue.get('version')!r}")
    if queue.get("branch") != BRANCH:
        raise RuntimeError(f"task queue targets {queue.get('branch')!r}, expected {BRANCH!r}")
    tasks = data.get("tasks", [])
    if not tasks:
        return []
    seen: set[str] = set()
    allowed_agents = {"harness", "opencode-review"}
    allowed_priorities = {"P0", "P1", "P2"}
    for task in tasks:
        tid = task.get("id")
        if not tid or tid in seen:
            raise RuntimeError(f"invalid or duplicate task id: {tid!r}")
        seen.add(tid)
        if task.get("agent") not in allowed_agents:
            raise RuntimeError(f"unsupported agent for {tid}: {task.get('agent')!r}")
        if task.get("priority") not in allowed_priorities:
            raise RuntimeError(f"invalid priority for {tid}: {task.get('priority')!r}")
        model = task.get("model")
        if model is not None and (not isinstance(model, str) or not model.strip()):
            raise RuntimeError(f"invalid model override for {tid}")
        if not task.get("title"):
            raise RuntimeError(f"task {tid} has no title")
        for check in task.get("verification", ["git-diff-check"]):
            if check not in VERIFICATION_CHECKS:
                raise RuntimeError(f"unknown verification check for {tid}: {check!r}")
    for task in tasks:
        tid = task["id"]
        deps = task.get("depends_on", [])
        if not isinstance(deps, list) or any(dep == tid or dep not in seen for dep in deps):
            raise RuntimeError(f"invalid dependency list for {tid}")

    graph = {task["id"]: task.get("depends_on", []) for task in tasks}
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(tid: str) -> None:
        if tid in visiting:
            raise RuntimeError(f"task dependency cycle detected at {tid}")
        if tid in visited:
            return
        visiting.add(tid)
        for dep in graph[tid]:
            visit(dep)
        visiting.remove(tid)
        visited.add(tid)

    for tid in graph:
        visit(tid)
    return tasks


def load_state() -> dict:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    if not STATE_FILE.exists():
        return {"version": 2, "tasks": {}, "updated_at": now()}
    return json.loads(STATE_FILE.read_text(encoding="utf-8"))


def save_state(state: dict) -> None:
    state["updated_at"] = now()
    write_heartbeat(state)
    tmp = STATE_FILE.with_suffix(".tmp")
    tmp.write_text(json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(STATE_FILE)


def write_heartbeat(state: dict) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    heartbeat = {
        "pid": os.getpid(),
        "updated_at": now(),
        "branch": BRANCH,
        "current_task": state.get("current_task"),
        "status": state.get("runner_status", "idle"),
    }
    tmp = HEARTBEAT_FILE.with_suffix(".tmp")
    tmp.write_text(json.dumps(heartbeat, indent=2) + "\n", encoding="utf-8")
    tmp.replace(HEARTBEAT_FILE)


def git_status() -> list[str]:
    return run(["git", "status", "--short"]).stdout.splitlines()


def git_head() -> str:
    return run(["git", "rev-parse", "HEAD"]).stdout.strip()


def git_tree_fingerprint() -> str:
    parts: list[bytes] = []
    for args in (["git", "status", "--short", "--untracked-files=all"], ["git", "diff", "--binary"], ["git", "diff", "--cached", "--binary"]):
        result = run(args)
        if result.returncode != 0:
            raise RuntimeError(f"git fingerprint command failed: {' '.join(args)}")
        parts.append(result.stdout.encode("utf-8"))
    untracked = run(["git", "ls-files", "--others", "--exclude-standard", "-z"])
    if untracked.returncode != 0:
        raise RuntimeError("git fingerprint command failed: git ls-files --others")
    for raw in untracked.stdout.split(chr(0)):
        if not raw:
            continue
        path = ROOT / raw
        try:
            payload = os.readlink(path).encode("utf-8") if path.is_symlink() else path.read_bytes()
        except OSError as exc:
            raise RuntimeError(f"cannot fingerprint untracked path: {raw}") from exc
        parts.append(raw.encode("utf-8") + bytes([0]) + hashlib.sha256(payload).digest())
    return hashlib.sha256(bytes([0]).join(parts)).hexdigest()

def require_expected_tree(info: dict) -> None:
    expected = info.get("expected_tree")
    if expected is None:
        return
    if git_tree_fingerprint() != expected:
        raise RuntimeError("working tree content changed outside the current AI turn; refusing autonomous resume")


def require_branch() -> None:
    current = run(["git", "branch", "--show-current"]).stdout.strip()
    if current != BRANCH:
        raise RuntimeError(f"expected branch {BRANCH!r}, got {current or '<detached>'!r}")


def check_not_paused() -> None:
    if PAUSE_FILE.exists():
        raise RuntimeError(f"manual pause requested: remove {PAUSE_FILE}")


def require_clean(reason: str) -> None:
    check_not_paused()
    status = git_status()
    if status:
        raise RuntimeError(f"working tree is not clean ({reason}); refusing autonomous mutation")


def next_task(tasks: list[dict], state: dict) -> dict | None:
    done = {tid for tid, info in state["tasks"].items() if info.get("status") == "done"}
    ready: list[dict] = []
    for task in tasks:
        info = state["tasks"].setdefault(task["id"], {"status": "pending", "attempts": 0})
        if info.get("status") != "pending":
            continue
        if any(dep not in done for dep in task.get("depends_on", [])):
            continue
        if info.get("attempts", 0) >= MAX_ATTEMPTS:
            info["status"] = "blocked"
            continue
        ready.append(task)
    ready.sort(key=lambda t: (0 if t["priority"] == "P0" else 1, t["id"]))
    return ready[0] if ready else None


def log_path(task_id: str, agent: str) -> Path:
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    return LOG_DIR / f"{stamp}-{task_id}-{agent}.log"


def resolve_dsh() -> str:
    candidate = shutil.which("dsh")
    if candidate:
        return candidate
    matches = list(Path.home().glob(".npm/_npx/*/node_modules/.bin/dsh"))
    if matches:
        matches.sort(key=lambda item: item.stat().st_mtime, reverse=True)
        return str(matches[0])
    raise RuntimeError("dsh CLI not found; install DeepSeek Harness first")


def resolve_opencode() -> str:
    candidate = shutil.which("opencode")
    if candidate:
        return candidate
    fallback = Path.home() / ".local/bin/opencode"
    if fallback.exists():
        return str(fallback)
    raise RuntimeError("opencode CLI not found")


def run_agent(agent: str, prompt: str, timeout: int, log: Path, model: str | None = None) -> int:
    env = os.environ.copy()
    if OFFLINE_MODE:
        env["OPENCODE_DISABLE_AUTOUPDATE"] = "1"
    if OPENCODE_CONFIG:
        env["OPENCODE_CONFIG"] = OPENCODE_CONFIG
    use_opencode = OFFLINE_MODE or agent == "opencode-review"
    if use_opencode:
        selected_model = model or OPENCODE_MODEL
        cmd = [resolve_opencode(), "run", "--auto", "--model", selected_model, prompt]
    elif agent == "harness":
        cmd = [resolve_dsh(), "--profile", "headless", prompt]
    else:
        raise RuntimeError(f"unknown agent: {agent}")
    with log.open("a", encoding="utf-8") as fh:
        fh.write(f"\n=== {now()} COMMAND ===\n{' '.join(cmd)}\n")
        if OPENCODE_CONFIG:
            fh.write(f"OPENCODE_CONFIG={OPENCODE_CONFIG}\n")
        proc = subprocess.Popen(
            cmd,
            cwd=ROOT,
            env=env,
            text=True,
            stdout=fh,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            rc = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            fh.write(f"\n=== TIMEOUT {timeout}s; terminating process group ===\n")
            try:
                os.killpg(proc.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                proc.wait()
            raise
        fh.write(f"\n=== EXIT {rc} ===\n")
    return rc


def prompt_for(task: dict, info: dict) -> tuple[str, int]:
    kind = task["agent"]
    filename = "implement.md" if kind == "harness" else "review.md"
    prompt = (HERE / "prompts" / filename).read_text(encoding="utf-8")
    prompt += "\n\nCURRENT TASK\n"
    prompt += f"ID: {task['id']}\nTITLE: {task['title']}\nPRIORITY: {task['priority']}\n"
    prompt += f"ATTEMPT: {info.get('attempts', 1)}/{MAX_ATTEMPTS}\n"
    prompt += f"AI TURN: {info.get('turns', 0) + 1}/{MAX_TURNS_PER_ATTEMPT}\n"
    acceptance = task.get("acceptance", [])
    if acceptance:
        prompt += "ACCEPTANCE CRITERIA\n"
        for criterion in acceptance:
            prompt += f"- {criterion}\n"
    if info.get("turns", 0):
        prompt += "This is a continuation of an earlier AI session. Preserve the existing work, inspect the current tree, and continue from where the previous session stopped. Do not restart or discard the implementation.\n"
    if info.get("verification_status") == "failed":
        prompt += "PREVIOUS VERIFICATION FAILED. Inspect the persisted verification results below, reproduce the failure, fix the implementation, and rerun the relevant checks before committing again.\n"
        for name, result in info.get("verification", {}).items():
            if result.get("status") == "fail":
                prompt += f"- {name}: {result.get('output_tail', '').strip()}\n"
    if info.get("phase") == "post-commit-dirty":
        prompt += "PREVIOUS SESSION CREATED A COMMIT BUT LEFT UNCOMMITTED CHANGES. Inspect those changes, decide whether they belong to this task, complete or revert them safely without destructive reset/clean operations, then create a focused follow-up commit before verification.\n"
    prompt += "Implement or review this task using the repository's current accepted architecture.\n"
    return prompt, HARNESS_TIMEOUT if kind == "harness" else OPENCODE_TIMEOUT


def verify_checks(task: dict, state: dict, info: dict, log: Path) -> bool:
    names = task.get("verification", ["git-diff-check"])
    info["verification"] = {name: {"status": "running"} for name in names}
    save_state(state)
    failed = False
    with log.open("a", encoding="utf-8") as fh:
        for name in names:
            started = time.monotonic()
            ok, output = run_check(name)
            elapsed = round(time.monotonic() - started, 3)
            info["verification"][name] = {
                "status": "pass" if ok else "fail",
                "checked_at": now(),
                "duration_seconds": elapsed,
                "output_tail": output[-4000:],
            }
            fh.write(f"\n=== VERIFY {name}: {'PASS' if ok else 'FAIL'} ===\n")
            fh.write(output.rstrip() + "\n")
            failed = failed or not ok
    info["verification_status"] = "passed" if not failed else "failed"
    save_state(state)
    return not failed


def find_task(tasks: list[dict], task_id: str) -> dict | None:
    return next((task for task in tasks if task["id"] == task_id), None)


def finalize_committed_task(state: dict, task: dict, info: dict) -> str:
    after_head = git_head()
    before_head = info.get("before_head")
    committed_head = info.get("committed_head")
    if before_head and after_head == before_head:
        return "pending"
    if committed_head and after_head != committed_head:
        raise RuntimeError("verified task commit no longer matches the repository HEAD")
    if git_status():
        raise RuntimeError("verified task has uncommitted changes in the working tree")
    if info.get("verification_status") != "passed":
        return "pending"
    info["status"] = "done"
    info["phase"] = "complete"
    info["last_result"] = "committed_and_verified"
    info["completed_at"] = now()
    info["after_head"] = after_head
    state["current_task"] = None
    state["runner_status"] = "idle"
    save_state(state)
    return "done"


def run_once(state: dict) -> str:
    require_branch()
    tasks = load_tasks()

    active_id = state.get("current_task")
    active = find_task(tasks, active_id) if active_id else None
    task = None
    if active is not None:
        info = state["tasks"].setdefault(active_id, {"status": "pending", "attempts": 1, "turns": 0})
        require_expected_tree(info)
        if info.get("attempts", 0) > MAX_ATTEMPTS:
            info["status"] = "blocked"
            state["current_task"] = None
            save_state(state)
            active = None
        elif info.get("phase") == "verification":
            if git_status():
                raise RuntimeError("committed task has an unexpected dirty tree before verification resume")
            state["runner_status"] = "verifying"
            save_state(state)
            log = log_path(active_id, f"verification-resume-{info.get('attempts', 0)}")
            if verify_checks(active, state, info, log):
                return finalize_committed_task(state, active, info)
            info["status"] = "pending" if info["attempts"] < MAX_ATTEMPTS else "blocked"
            info["phase"] = "verification-failed"
            info["last_result"] = "verification_failed_on_resume"
            if info["status"] == "blocked":
                state["current_task"] = None
            save_state(state)
            return "failed"
        elif info.get("phase") == "verification-failed":
            require_clean("before verification remediation")
            task = active
        else:
            current_tree = git_tree_fingerprint()
            current_status = git_status()
            current_head = git_head()
            if not current_status and info.get("before_head") and current_head != info["before_head"]:
                info["committed_head"] = current_head
                info["expected_tree"] = current_tree
                info["phase"] = "verification"
                state["runner_status"] = "verifying"
                save_state(state)
                log = log_path(active_id, f"verification-recover-{info.get('attempts', 0)}")
                if verify_checks(active, state, info, log):
                    return finalize_committed_task(state, active, info)
                info["status"] = "pending" if info["attempts"] < MAX_ATTEMPTS else "blocked"
                info["phase"] = "verification-failed"
                info["last_result"] = "verification_failed_after_recovery"
                if info["status"] == "blocked":
                    state["current_task"] = None
                save_state(state)
                return "failed"
            task = active
    else:
        task = None

    if task is None:
        require_clean("before new task start")
        task = next_task(tasks, state)
        save_state(state)
        if task is None:
            state["runner_status"] = "idle"
            save_state(state)
            return "idle"
        tid = task["id"]
        info = state["tasks"][tid]
        info["attempts"] = int(info.get("attempts", 0)) + 1
        info["turns"] = 0
        info["phase"] = "implementation" if task["agent"] == "harness" else "review"
        info["status"] = "running"
        info["started_at"] = now()
        info["before_head"] = git_head()
        info["expected_tree"] = git_tree_fingerprint()
        state["current_task"] = tid
        state["runner_status"] = "working"
        save_state(state)
    else:
        tid = task["id"]
        info = state["tasks"][tid]
        info["status"] = "running"
        state["runner_status"] = "working"
        save_state(state)

    if int(info.get("turns", 0)) >= MAX_TURNS_PER_ATTEMPT:
        info["status"] = "pending" if int(info.get("attempts", 0)) < MAX_ATTEMPTS else "blocked"
        info["phase"] = "turn-limit"
        info["last_result"] = "max_ai_turns_reached"
        if info["status"] == "blocked":
            state["current_task"] = None
        save_state(state)
        return "failed"

    info["turns"] = int(info.get("turns", 0)) + 1
    save_state(state)
    prompt, timeout = prompt_for(task, info)
    log = log_path(tid, task["agent"])
    try:
        rc = run_agent(task["agent"], prompt, timeout, log, task.get("model"))
    except subprocess.TimeoutExpired:
        status = git_status()
        dirty = bool(status)
        info["expected_tree"] = git_tree_fingerprint()
        info["status"] = "pending" if info["attempts"] < MAX_ATTEMPTS else "blocked"
        info["phase"] = "timeout-resume" if dirty else "timeout"
        info["last_result"] = "timeout_resume" if dirty else "timeout"
        if info["status"] == "blocked":
            state["current_task"] = None
        save_state(state)
        return "failed"

    if rc != 0:
        status = git_status()
        dirty = bool(status)
        info["expected_tree"] = git_tree_fingerprint()
        info["status"] = "pending" if info["attempts"] < MAX_ATTEMPTS else "blocked"
        info["phase"] = "agent-exit-resume" if dirty else "agent-exit"
        info["last_result"] = f"exit_{rc}_resume" if dirty else f"exit_{rc}"
        if info["status"] == "blocked":
            state["current_task"] = None
        save_state(state)
        return "failed"

    after_head = git_head()
    status = git_status()
    if after_head == info["before_head"]:
        info["expected_tree"] = git_tree_fingerprint()
        dirty = bool(status)
        info["status"] = "pending" if info["attempts"] < MAX_ATTEMPTS else "blocked"
        info["phase"] = "no-commit-resume" if dirty else "no-commit"
        info["last_result"] = "agent_needs_continuation" if dirty else "agent_created_no_commit"
        if info["status"] == "blocked":
            state["current_task"] = None
        save_state(state)
        return "failed"

    status = git_status()
    info["committed_head"] = after_head
    info["expected_tree"] = git_tree_fingerprint()
    if status:
        info["phase"] = "post-commit-dirty"
        info["status"] = "pending"
        info["last_result"] = "commit_left_uncommitted_changes"
        save_state(state)
        return "failed"
    info["phase"] = "verification"
    state["runner_status"] = "verifying"
    save_state(state)
    if not verify_checks(task, state, info, log):
        info["status"] = "pending" if info["attempts"] < MAX_ATTEMPTS else "blocked"
        info["last_result"] = "verification_failed"
        info["phase"] = "verification-failed"
        if info["status"] == "blocked":
            state["current_task"] = None
        save_state(state)
        return "failed"

    return finalize_committed_task(state, task, info)


def acquire_lock():
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    fh = LOCK_FILE.open("w", encoding="utf-8")
    try:
        fcntl.flock(fh.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as exc:
        raise RuntimeError("another Luna Agent runner is already active") from exc
    return fh


def pid_alive(pid: int | None) -> bool:
    if not pid:
        return False
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


def effective_status(state: dict) -> str:
    if PAUSE_FILE.exists():
        return "paused"
    raw = state.get("runner_status", "idle")
    if raw in {"starting", "working", "recovering", "verifying"}:
        heartbeat = {}
        if HEARTBEAT_FILE.exists():
            try:
                heartbeat = json.loads(HEARTBEAT_FILE.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                heartbeat = {}
        if not pid_alive(heartbeat.get("pid")):
            return "stale"
    return raw


def doctor() -> int:
    failures = []
    warnings = []
    try:
        require_branch()
    except RuntimeError as exc:
        failures.append(str(exc))
    try:
        tasks = load_tasks()
        print(f"queue: OK ({len(tasks)} tasks)")
    except Exception as exc:
        failures.append(f"queue: {exc}")
    if OFFLINE_MODE:
        print("mode: offline/local")
        print(f"local model: {OPENCODE_MODEL}")
        if not OPENCODE_CONFIG:
            warnings.append("offline mode is using the normal OpenCode config; set LUNA_AGENT_OPENCODE_CONFIG for a dedicated local profile")
        elif not Path(OPENCODE_CONFIG).is_file():
            failures.append(f"local OpenCode config not found: {OPENCODE_CONFIG}")
        try:
            print(f"opencode: {resolve_opencode()}")
        except RuntimeError as exc:
            failures.append(str(exc))
    else:
        for label, resolver in (("dsh", resolve_dsh), ("opencode", resolve_opencode)):
            try:
                print(f"{label}: {resolver()}")
            except RuntimeError as exc:
                failures.append(str(exc))
    status = git_status()
    if status:
        warnings.append(f"working tree is dirty ({len(status)} status entries)")
    else:
        print("working tree: clean")
    unit = Path.home() / ".config/systemd/user/project-luna-agent.service"
    print(f"systemd unit: {'installed' if unit.exists() else 'not installed'}")
    if PAUSE_FILE.exists():
        warnings.append(f"manual pause marker exists: {PAUSE_FILE}")
    for warning in warnings:
        print(f"warning: {warning}", file=sys.stderr)
    for failure in failures:
        print(f"error: {failure}", file=sys.stderr)
    return 2 if failures else 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Project Luna sequential agent runner")
    parser.add_argument("--once", action="store_true", help="process one AI turn")
    parser.add_argument("--continuous", action="store_true", help="process tasks continuously")
    parser.add_argument("--status", action="store_true", help="show durable runner/task state")
    parser.add_argument("--doctor", action="store_true", help="check local agent prerequisites without starting work")
    args = parser.parse_args()
    if args.status:
        state = load_state()
        payload = dict(state)
        payload["effective_status"] = effective_status(state)
        payload["pause_marker"] = str(PAUSE_FILE) if PAUSE_FILE.exists() else None
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    if args.doctor:
        return doctor()
    if args.once == args.continuous:
        parser.error("choose exactly one of --once or --continuous")

    lock = acquire_lock()
    state = load_state()
    state["runner_status"] = "starting"
    save_state(state)

    if args.once:
        try:
            result = run_once(state)
        except Exception as exc:
            state["runner_status"] = "paused"
            state["runner_error"] = str(exc)
            state["runner_error_at"] = now()
            save_state(state)
            print(f"Luna Agent paused: {exc}", file=sys.stderr)
            return 2
        print(result)
        return 0 if result in {"done", "idle"} else 2

    while True:
        try:
            result = run_once(state)
            if result == "idle":
                time.sleep(60)
            elif result == "done":
                time.sleep(5)
            else:
                time.sleep(30)
        except KeyboardInterrupt:
            return 130
        except Exception as exc:
            state["runner_error"] = str(exc)
            state["runner_error_at"] = now()
            state["runner_status"] = "paused"
            save_state(state)
            print(f"Luna Agent paused: {exc}", file=sys.stderr)
            time.sleep(60)


if __name__ == "__main__":
    raise SystemExit(main())
