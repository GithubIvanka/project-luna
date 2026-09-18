# Local AI backend for Luna Agent

Luna Agent can use OpenCode with any local OpenAI-compatible inference server.
The model is selected by `LUNA_AGENT_OPENCODE_MODEL` or a task-level `model`
override. A custom OpenCode config can be supplied with
`LUNA_AGENT_OPENCODE_CONFIG`.

## Offline mode

Set `LUNA_AGENT_OFFLINE=1` when the runner must not invoke DeepSeek Harness.
OpenCode is then expected to use a local provider. The runner also disables
OpenCode automatic update checks for that process.

Example environment:

```text
LUNA_AGENT_OFFLINE=1
LUNA_AGENT_OPENCODE_MODEL=local/ornith
LUNA_AGENT_OPENCODE_CONFIG=/home/user/.config/project-luna/opencode-local.jsonc
```

The model weights and inference runtime must already exist on the machine.
No cloud API key is needed for an unauthenticated local endpoint.

## Two-node design

Use one coordinator and one worker/reviewer. Do not share one working tree
between machines while agents are editing it. Exchange Git commits over SSH.
The coordinator keeps task state; each remote worker operates in its own clone
or worktree and returns a commit for verification.
