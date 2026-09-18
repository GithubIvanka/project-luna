# Two-node local AI topology

The preferred Alpha topology is a coordinator plus one inference/worker node.
Both machines stay on a private LAN. Internet access is optional and can be
removed after model weights, runtimes, and source dependencies are available.

```text
PC-A: Luna development host
  Luna Agent -> Git task queue -> verification -> QEMU/build
       |
       +---- SSH/Git ----> PC-B: local AI worker
                              |
                              +-- llama.cpp / vLLM / Ollama
                              +-- Ornith or another local coding model
```

PC-A should keep the canonical repository and verification authority. PC-B
should use its own clone/worktree and return commits; it must not edit PC-A's
working tree through a network mount.

For a first implementation, expose an OpenAI-compatible inference endpoint on
PC-B over the private LAN. OpenCode on PC-A can use that endpoint as a custom
local provider. Restrict the port with the LAN firewall and prefer SSH for Git.

This design removes cloud token limits from implementation and review turns.
CPU/RAM limits and model context still remain real local limits.

A later optimization can reserve PC-B for heavy inference while PC-A runs a
small local model for triage, summarization, and lightweight review tasks.
