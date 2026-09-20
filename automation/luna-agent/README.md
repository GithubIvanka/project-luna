# Project Luna Agent

This directory contains the local sequential orchestrator for Luna development.

The runner coordinates two local coding agents without allowing them to edit
one working tree concurrently. Harness is the primary implementation worker;
OpenCode is the independent review/fix worker.

## Execution model

```text
Task queue
  -> Harness implementation
  -> agent commit
  -> OpenCode review/fix
  -> agent commit
  -> verification
  -> next task
```

The runner requires `alpha-development` and a clean working tree before every
agent transaction. It never runs `git reset`, `git clean`, or other cleanup that
could discard work. If another process changes the repository, the runner
pauses rather than taking ownership of those changes.

## Files

- `tasks.toml` — immutable task definitions and dependencies.
- `state.toml` — legacy/bootstrap state file; active runtime state is stored
  outside the repository at `~/.local/state/project-luna/luna-agent/state.json`.
- `prompts/` — implementation and review instructions.
- `logs/` — ignored runtime logs.
- `runner.py` — sequential runner.

## Commands

```bash
python3 automation/luna-agent/runner.py --doctor
python3 automation/luna-agent/runner.py --status
python3 automation/luna-agent/runner.py --once
python3 automation/luna-agent/runner.py --continuous
```

`--doctor` checks the branch, task queue, agent CLIs, and local service setup
without starting autonomous work. Continuous mode is intended for long
unattended development sessions. It sleeps between tasks and preserves task
attempts and runner errors externally.
Task acceptance criteria are stored in `tasks.toml` and injected into every AI
turn so an agent has explicit completion conditions instead of only a title.
The queue format is versioned and validated before work begins.

## Verification

Every task runs an allow-listed verification phase after the agent reports success.
See `VERIFICATION.md` for the checks and `verification.py` for the implementation.
A failed check keeps the task resumable instead of marking it done.

## Safety

The runner is not an installer and does not authorize physical-disk changes,
host reboot, user-data deletion, or destructive recovery operations.

Agents must follow the repository `AGENTS.md` and `.agents/skills/` contracts.

## Resumable AI turns

A task is allowed to span several independent Harness/OpenCode sessions.
The runner keeps `attempts` and `turns` in `~/.local/state/project-luna/luna-agent/state.json`.
When an AI session exits, times out, or finishes without committing but leaves
work in the tree, the next turn resumes the same task instead of restarting it.
The runner records the expected working-tree status between turns; an external
change pauses autonomous work instead of being mixed into an AI continuation.
After a commit, verification is a durable phase. A runner restart repeats an
interrupted verification, while a failed verification is passed to a fresh AI
turn for remediation. Each task attempt allows up to six AI turns and three
full attempts.

## Status and heartbeat

```bash
python3 automation/luna-agent/runner.py --status
```

Runtime state and `heartbeat.json` live outside Git. Agent output is stored in
`automation/luna-agent/logs/`, which is ignored.

## User systemd service

The service definition is `systemd/project-luna-agent.service`. Install it only
after the human/interactive development session has finished and the working
 tree is clean:

```bash
mkdir -p ~/.config/systemd/user
cp automation/luna-agent/systemd/project-luna-agent.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now project-luna-agent.service
systemctl --user status project-luna-agent.service
```

Stop it with `systemctl --user disable --now project-luna-agent.service`.

To pause a running service without editing the repository, create the external
marker `~/.local/state/project-luna/luna-agent/PAUSE`. The runner will stop
starting/resuming work; remove the marker to continue.

## Resilient free backend

The `free` backend routes every AI turn through independent provider scopes;
it does not treat several free models behind one gateway as separate quota
pools. The default order is:

```text
Inception/Mercury 2.5
  -> Gemini 2.5 Pro
  -> OpenCode Zen free models
  -> Cerebras GPT-OSS 120B
  -> OpenRouter free models
  -> Mistral (when a model is configured)
  -> Atria (only when its API contract is explicitly configured)
  -> local Ollama / Ornith 1.5 9B
```

A quota, rate-limit, authentication failure, or provider outage cools the
whole provider scope and the runner resumes the same task/turn on the next
independent scope. Task state, Git state, acceptance criteria, skills, and
verification evidence are unchanged by a provider switch.

Provider credentials are loaded from the user-owned file
`~/.config/project-luna/luna-agent/providers.env`. Never put API keys in Git.
Start from `automation/luna-agent/providers.env.example` and use
`chmod 600` on the private copy. Missing providers are skipped automatically.

The free pool is implemented through Harness/DSH's OpenAI-compatible provider
support, so the critical path does not depend on the unreliable OpenCode
standalone process. OpenCode remains available for the separate review worker.

## Runtime tuning

The defaults are conservative, but unattended runs can override them without
editing the repository: `LUNA_AGENT_MAX_ATTEMPTS`, `LUNA_AGENT_MAX_TURNS`,
`LUNA_AGENT_HARNESS_TIMEOUT`, `LUNA_AGENT_OPENCODE_TIMEOUT`,
`LUNA_AGENT_FREE_TIMEOUT`, and `LUNA_AGENT_FREE_RETRY_SECONDS`.

The OpenCode review worker uses `opencode-direct.sh`, which reads the existing
OpenRouter credential from OpenCode's local auth store without writing the key
to the repository or logs. If OpenCode exits unsuccessfully or times out, the
runner terminates it and continues the review with a fresh Harness turn instead
of idling the queue.
