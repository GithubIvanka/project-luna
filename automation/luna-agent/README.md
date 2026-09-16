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
python3 automation/luna-agent/runner.py --once
python3 automation/luna-agent/runner.py --continuous
```

Continuous mode is intended for long unattended development sessions. It
sleeps between tasks and preserves task attempts and runner errors externally.

## Safety

The runner is not an installer and does not authorize physical-disk changes,
host reboot, user-data deletion, or destructive recovery operations.

Agents must follow the repository `AGENTS.md` and `.agents/skills/` contracts.

## Resumable AI turns

A task is allowed to span several independent Harness/OpenCode sessions.
The runner keeps `attempts` and `turns` in `~/.local/state/project-luna/luna-agent/state.json`.
When an AI session exits, times out, or finishes without committing but leaves
work in the tree, the next turn resumes the same task instead of restarting it.
Each task attempt allows up to six AI turns and three full attempts.

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
