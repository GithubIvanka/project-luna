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
from free_backend import FREE_POOL, Provider, prepare_dsh, provider_available, provider_entries, provider_for

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
TASKS_FILE = HERE / "tasks.toml"
LOG_DIR = HERE / "logs"
STATE_DIR = Path.home() / ".local/state/project-luna/luna-agent"
STATE_FILE = STATE_DIR / "state.json"
LOCK_FILE = STATE_DIR / "runner.lock"
HEARTBEAT_FILE = STATE_DIR / "heartbeat.json"
PAUSE_FILE = STATE_DIR / "PAUSE"
HANDOFF_FILE = STATE_DIR / "HANDOFF_READY.json"
BRANCH = "alpha-development"
MAX_ATTEMPTS = int(os.environ.get("LUNA_AGENT_MAX_ATTEMPTS", "3"))
MAX_TURNS_PER_ATTEMPT = int(os.environ.get("LUNA_AGENT_MAX_TURNS", "6"))
HARNESS_TIMEOUT = int(os.environ.get("LUNA_AGENT_HARNESS_TIMEOUT", "3600"))
OPENCODE_TIMEOUT = int(os.environ.get("LUNA_AGENT_OPENCODE_TIMEOUT", "180"))
FREE_TIMEOUT = int(os.environ.get("LUNA_AGENT_FREE_TIMEOUT", "120"))
FREE_RETRY_SECONDS = int(os.environ.get("LUNA_AGENT_FREE_RETRY_SECONDS", "300"))
OPENCODE_MODEL = os.environ.get("LUNA_AGENT_OPENCODE_MODEL", "openrouter/deepseek/deepseek-v4-flash-0731#high")
OPENCODE_CONFIG = os.environ.get("LUNA_AGENT_OPENCODE_CONFIG")
BACKEND = os.environ.get("LUNA_AGENT_BACKEND", "online")
OFFLINE_MODE = os.environ.get("LUNA_AGENT_OFFLINE", "0") == "1"
LOCAL_MODEL = os.environ.get("LUNA_AGENT_LOCAL_MODEL", "ornith-1.5:9b")
DSH_FREE_HOME = STATE_DIR / "dsh-free"
TASKS_VERSION = 2


def run(cmd: list[str], timeout: int | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, timeout=timeout)


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def free_pool_entry(index: int) -> Provider:
    return provider_for(index)


def free_pool_scope(entry: Provider) -> str:
    return entry.scope


def free_pool_size() -> int:
    return len(FREE_POOL)


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
        return {"version": 2, "tasks": {}, "autonomy_authorized": False, "updated_at": now()}
    state = json.loads(STATE_FILE.read_text(encoding="utf-8"))
    state.setdefault("autonomy_authorized", False)
    return state


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
        "backend": BACKEND,
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


def create_handoff() -> None:
    require_branch()
    require_clean("creating autonomous handoff")
    payload = {"branch": BRANCH, "head": git_head(), "created_at": now()}
    tmp = HANDOFF_FILE.with_suffix(".tmp")
    tmp.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    tmp.replace(HANDOFF_FILE)


def require_handoff() -> None:
    if not HANDOFF_FILE.exists():
        raise RuntimeError(f"waiting for development handoff: create {HANDOFF_FILE}")
    try:
        payload = json.loads(HANDOFF_FILE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeError("autonomous handoff marker is invalid") from exc
    if payload.get("branch") != BRANCH or payload.get("head") != git_head():
        raise RuntimeError("autonomous handoff marker does not match the current repository HEAD")
    require_clean("after autonomous handoff")
    HANDOFF_FILE.unlink()


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
    wrapper = HERE / "opencode-direct.sh"
    if wrapper.exists() and os.access(wrapper, os.X_OK):
        return str(wrapper)
    candidate = shutil.which("opencode")
    if candidate:
        return candidate
    fallback = Path.home() / ".local/bin/opencode"
    if fallback.exists():
        return str(fallback)
    raise RuntimeError("opencode CLI not found")


def is_free_model(model: str | None) -> bool:
    return bool(model and any(provider.model == model for provider in FREE_POOL))


def pool_entry_available(entry: Provider, state: dict | None = None) -> bool:
    cooldowns = (state or {}).get("provider_cooldowns", {})
    return provider_available(entry, cooldowns)


def next_free_pool_index(current: int, *, skip_scope: str | None = None, state: dict | None = None) -> int | None:
    for step in range(1, free_pool_size() + 1):
        idx = (current + step) % free_pool_size()
        entry = free_pool_entry(idx)
        if skip_scope and free_pool_scope(entry) == skip_scope:
            continue
        if pool_entry_available(entry, state):
            return idx
    return None


def prepare_free_harness(env: dict[str, str], provider: Provider) -> None:
    if not is_free_model(provider.model):
        raise RuntimeError(f"selected provider is not in the free pool: {provider.model!r}")
    prepare_dsh(env, DSH_FREE_HOME, provider)


def defer_free_backend_retry(state: dict, info: dict, reason: str) -> bool:
    if BACKEND != "free" or git_status():
        return False
    info["turns"] = max(0, int(info.get("turns", 0)) - 1)
    info["status"] = "pending"
    info["phase"] = "backend-wait"
    info["last_result"] = reason
    state["backend_wait_until"] = time.time() + FREE_RETRY_SECONDS
    state["runner_status"] = "waiting-backend"
    save_state(state)
    return True


def classify_free_failure(log: Path) -> tuple[str, str] | None:
    try:
        text = log.read_text(encoding="utf-8", errors="replace")[-16000:].lower()
    except OSError:
        return None
    global_markers = (
        "free-models-per-day",
        "openrouter_free_tier_daily",
        'x-ratelimit-remaining\\":\\"0',
        "daily ceiling",
    )
    if any(marker in text for marker in global_markers):
        return ("provider", "global-rate-limit")
    transient_markers = (
        "rate limit",
        "rate_limit",
        "too many requests",
        "quota exceeded",
        "resource exhausted",
        "temporarily unavailable",
        "service unavailable",
        "overloaded",
        "capacity",
        "status code 429",
        "http 429",
        "http 503",
        "status code 402",
        "http 402",
        " code 429",
        " code 503",
        "insufficient credits",
        "payment required",
    )
    auth_markers = (
        "unauthorized",
        "authentication failed",
        "invalid api key",
        "no provider available",
        "api key is required",
        "missing api key",
        "status code 401",
        "status code 403",
        "http 401",
        "http 403",
    )
    context_markers = (
        "context length exceeded",
        "maximum context length",
        "context window exceeded",
        "token limit exceeded",
        "too many tokens",
    )
    if any(marker in text for marker in auth_markers):
        return ("auth", "provider-unavailable")
    model_markers = context_markers + (
        "model not found",
        "model was not found",
        "unknown model",
        "invalid model",
        "model does not exist",
    )
    if any(marker in text for marker in model_markers):
        return ("model", "model-limit")
    if any(marker in text for marker in transient_markers):
        reason = "quota-exhausted" if any(marker in text for marker in ("quota exceeded", "resource exhausted", "free-models-per-day", "insufficient credits")) else "provider-rate-limit"
        return ("provider", reason)
    return None


def advance_free_pool(state: dict, info: dict, log: Path, forced_reason: str | None = None) -> bool:
    failure = classify_free_failure(log)
    if BACKEND != "free" or (failure is None and forced_reason is None) or git_status():
        return False
    current = int(info.get("free_pool_index", 0))
    current_entry = free_pool_entry(current)
    scope, reason = failure or ("provider", forced_reason or "provider-failure")
    failed_scope = free_pool_scope(current_entry) if scope in {"provider", "openrouter"} else None
    if scope == "auth":
        failed_scope = free_pool_scope(current_entry)
    if failed_scope:
        cooldown_seconds = 86_400 if reason in {"global-rate-limit", "quota-exhausted"} else FREE_RETRY_SECONDS
        if scope == "auth":
            cooldown_seconds = max(FREE_RETRY_SECONDS, 3_600)
        state.setdefault("provider_cooldowns", {})[failed_scope] = time.time() + cooldown_seconds
    next_index = next_free_pool_index(current, skip_scope=failed_scope, state=state)
    info["turns"] = max(0, int(info.get("turns", 0)) - 1)
    if next_index is None:
        info["status"] = "pending"
        info["phase"] = "backend-wait"
        info["last_result"] = reason
        state["backend_wait_until"] = time.time() + FREE_RETRY_SECONDS
        state["runner_status"] = "waiting-backend"
        save_state(state)
        return True
    entry = free_pool_entry(next_index)
    info["free_pool_index"] = next_index
    info["selected_runner"] = "harness"
    info["selected_model"] = entry.model
    info["status"] = "pending"
    info["phase"] = "model-fallback"
    info["last_result"] = f"{reason}:{current_entry.scope}:{current_entry.model} -> {entry.scope}:{entry.model}"
    state["runner_status"] = "working"
    save_state(state)
    return True


def run_agent(
    agent: str,
    prompt: str,
    timeout: int,
    log: Path,
    model: str | None = None,
    free_runner: str | None = None,
) -> int:
    env = os.environ.copy()
    # Let DSH Agent Skills discover the project's canonical .agents/skills tree.
    env["DSH_AGENTS_HOME"] = str(ROOT / ".agents")
    if BACKEND not in {"online", "free", "local"}:
        raise RuntimeError(f"unsupported Luna Agent backend: {BACKEND}")
    local_mode = OFFLINE_MODE or BACKEND == "local"
    free_mode = BACKEND == "free"
    if local_mode:
        env["OPENCODE_DISABLE_AUTOUPDATE"] = "1"
    if OPENCODE_CONFIG:
        env["OPENCODE_CONFIG"] = OPENCODE_CONFIG
    if local_mode:
        selected_model = model if model and model.startswith("local/") else f"local/{LOCAL_MODEL.removeprefix('local/')}"
        cmd = [resolve_opencode(), "run", "--standalone", "--auto", "--model", selected_model, prompt]
    elif free_mode:
        if not is_free_model(model):
            raise RuntimeError(f"free backend requires a selected pool model, got {model!r}")
        selected_model = model
        provider = next(provider for provider in FREE_POOL if provider.model == selected_model)
        timeout = min(timeout, FREE_TIMEOUT)
        prepare_free_harness(env, provider)
        cmd = [resolve_dsh(), "--profile", "headless", prompt]
    elif agent == "opencode-review":
        selected_model = model or OPENCODE_MODEL
        cmd = [resolve_opencode(), "run", "--standalone", "--auto", "--model", selected_model, prompt]
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
        timed_out = False
        try:
            rc = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            rc = -signal.SIGTERM
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

        if agent == "opencode-review" and (timed_out or rc != 0) and not free_mode:
            reason = "timeout" if timed_out else f"exit {rc}"
            fh.write(f"\n=== OPENCODE FALLBACK ({reason}) -> HARNESS REVIEW ===\n")
            fallback_prompt = (
                prompt
                + "\n\nOpenCode review worker was unavailable. "
                "Perform this review as a fresh independent pass with the Harness worker. "
                "Do not assume the previous review succeeded; inspect the implementation and report concrete findings.\n"
            )
            fallback_cmd = [resolve_dsh(), "--profile", "headless", fallback_prompt]
            fh.write(f"=== FALLBACK COMMAND ===\n{' '.join(fallback_cmd)}\n")
            fallback = subprocess.Popen(
                fallback_cmd,
                cwd=ROOT,
                env=env,
                text=True,
                stdout=fh,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            try:
                rc = fallback.wait(timeout=HARNESS_TIMEOUT)
            except subprocess.TimeoutExpired:
                fh.write(f"\n=== FALLBACK TIMEOUT {HARNESS_TIMEOUT}s; terminating process group ===\n")
                try:
                    os.killpg(fallback.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    fallback.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(fallback.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    fallback.wait()

        fh.write(f"\n=== EXIT {rc} ===\n")
    return rc


def first_free_pool_index(state: dict) -> int | None:
    for index in range(free_pool_size()):
        if pool_entry_available(free_pool_entry(index), state):
            return index
    return None


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
    if info.get("phase") == "model-fallback":
        prompt += f"PREVIOUS FREE PROVIDER FAILED. Continue the same logical AI turn on the next provider/model. Previous provider result: {info.get('last_result', 'unknown')}. Preserve the existing task scope and acceptance criteria; do not restart or widen the task.\n"
    prompt += "Implement or review this task using the repository's current accepted architecture.\n"
    if BACKEND == "free":
        timeout = FREE_TIMEOUT
    elif BACKEND == "local" or OFFLINE_MODE:
        timeout = OPENCODE_TIMEOUT
    else:
        timeout = HARNESS_TIMEOUT if kind == "harness" else OPENCODE_TIMEOUT
    return prompt, timeout


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

    if BACKEND == "free":
        wait_until = float(state.get("backend_wait_until", 0))
        if wait_until and time.time() < wait_until:
            state["runner_status"] = "waiting-backend"
            save_state(state)
            return "backend-wait"
        if wait_until:
            state.pop("backend_wait_until", None)

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
        if not state.get("autonomy_authorized", False):
            require_handoff()
            state["autonomy_authorized"] = True
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
        if BACKEND == "free":
            requested_model = task.get("model")
            if requested_model and any(item.model == requested_model for item in FREE_POOL):
                pool_index = next(i for i, item in enumerate(FREE_POOL) if item.model == requested_model)
            else:
                pool_index = first_free_pool_index(state)
            if pool_index is None:
                info["status"] = "pending"
                info["phase"] = "backend-wait"
                info["last_result"] = "no_free_provider_available"
                state["backend_wait_until"] = time.time() + FREE_RETRY_SECONDS
                state["runner_status"] = "waiting-backend"
                state["current_task"] = tid
                save_state(state)
                return "backend-wait"
            entry = free_pool_entry(pool_index)
            info["free_pool_index"] = pool_index
            info["selected_runner"] = "harness"
            info["selected_provider"] = entry.id
            info["selected_scope"] = entry.scope
            info["selected_model"] = entry.model
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
        # A turn limit ends the current durable attempt. Clear current_task so
        # the next loop selects the pending task again and increments attempts.
        state["current_task"] = None
        save_state(state)
        return "failed"

    if BACKEND == "free":
        current_index = int(info.get("free_pool_index", 0))
        current_entry = free_pool_entry(current_index)
        if not pool_entry_available(current_entry, state):
            next_index = next_free_pool_index(current_index - 1, state=state)
            if next_index is None:
                info["status"] = "pending"
                info["phase"] = "backend-wait"
                info["last_result"] = "selected_free_provider_unavailable"
                state["backend_wait_until"] = time.time() + FREE_RETRY_SECONDS
                state["runner_status"] = "waiting-backend"
                save_state(state)
                return "backend-wait"
            info["free_pool_index"] = next_index
            next_entry = free_pool_entry(next_index)
            info["selected_provider"] = next_entry.id
            info["selected_scope"] = next_entry.scope
            info["selected_model"] = next_entry.model
            info["selected_runner"] = "harness"
            info["phase"] = "model-fallback"
            info["last_result"] = f"provider-unavailable:{current_entry.scope}:{current_entry.model} -> {next_entry.scope}:{next_entry.model}"
            save_state(state)
    info["turns"] = int(info.get("turns", 0)) + 1
    save_state(state)
    prompt, timeout = prompt_for(task, info)
    selected_model = info.get("selected_model") if BACKEND == "free" else task.get("model")
    selected_runner = "harness" if BACKEND == "free" else None
    if BACKEND == "free" and not selected_model:
        entry = next((item for item in FREE_POOL if pool_entry_available(item, state)), free_pool_entry(0))
        selected_model = entry.model
        selected_runner = "harness"
        info["free_pool_index"] = FREE_POOL.index(entry)
        info["selected_provider"] = entry.id
        info["selected_scope"] = entry.scope
        info["selected_model"] = selected_model
        info["selected_runner"] = selected_runner
        save_state(state)
    log = log_path(tid, task["agent"])
    try:
        rc = run_agent(task["agent"], prompt, timeout, log, selected_model, selected_runner)
    except subprocess.TimeoutExpired:
        status = git_status()
        dirty = bool(status)
        info["expected_tree"] = git_tree_fingerprint()
        if advance_free_pool(state, info, log, "provider-timeout"):
            return "model-fallback"
        if defer_free_backend_retry(state, info, "free_backend_timeout"):
            return "backend-wait"
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
        if advance_free_pool(state, info, log):
            return "model-fallback"
        if defer_free_backend_retry(state, info, f"free_backend_exit_{rc}"):
            return "backend-wait"
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
    print(f"backend: {BACKEND}")
    if BACKEND == "local" or OFFLINE_MODE:
        print(f"local model: {LOCAL_MODEL}")
        if not OPENCODE_CONFIG:
            warnings.append("local mode is using the normal OpenCode config; set LUNA_AGENT_OPENCODE_CONFIG for a dedicated local profile")
        elif not Path(OPENCODE_CONFIG).is_file():
            failures.append(f"local OpenCode config not found: {OPENCODE_CONFIG}")
    elif BACKEND == "free":
        print("free pool:")
        for entry in provider_entries():
            marker = "OK" if entry["available"] else "skip"
            suffix = " (experimental)" if entry["experimental"] else ""
            print(f"  {marker}: {entry['scope']} / {entry['model']}{suffix}")
    if BACKEND == "free":
        try:
            print(f"dsh: {resolve_dsh()}")
        except RuntimeError as exc:
            failures.append(str(exc))
    elif BACKEND == "local" or OFFLINE_MODE:
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
    parser.add_argument("--handoff", action="store_true", help="authorize autonomous work from the current clean HEAD")
    args = parser.parse_args()
    if args.status:
        state = load_state()
        payload = dict(state)
        payload["effective_status"] = effective_status(state)
        payload["backend"] = BACKEND
        active_info = payload.get("tasks", {}).get(payload.get("current_task"), {}) if payload.get("current_task") else {}
        payload["model"] = (
            LOCAL_MODEL if BACKEND == "local"
            else active_info.get("selected_model", FREE_POOL[0].model) if BACKEND == "free"
            else OPENCODE_MODEL
        )
        payload["pause_marker"] = str(PAUSE_FILE) if PAUSE_FILE.exists() else None
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    if args.doctor:
        return doctor()
    if args.handoff:
        try:
            create_handoff()
        except Exception as exc:
            print(f"Luna Agent handoff refused: {exc}", file=sys.stderr)
            return 2
        print(HANDOFF_FILE)
        return 0
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
            elif result == "backend-wait":
                time.sleep(60)
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
