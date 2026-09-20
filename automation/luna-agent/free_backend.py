"""Provider registry and credential/setup helpers for Luna's resilient free backend."""
from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
import shlex
import subprocess
import time

PROVIDER_ENV_FILE = Path.home() / ".config/project-luna/luna-agent/providers.env"
OPENROUTER_AUTH = Path.home() / ".local/share/opencode/auth.json"
DEFAULT_OLLAMA_BIN = Path.home() / ".local/bin/ollama"


@dataclass(frozen=True)
class Provider:
    id: str
    scope: str
    model: str
    base_url: str
    api_key_env: str
    context_window: int
    max_tokens: int
    reasoning_effort: str | None = None
    experimental: bool = False


def load_provider_env() -> None:
    if not PROVIDER_ENV_FILE.is_file():
        return
    try:
        lines = PROVIDER_ENV_FILE.read_text(encoding="utf-8").splitlines()
    except OSError:
        return
    for line in lines:
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip()
        if key and value and key not in os.environ:
            try:
                value = shlex.split(value)[0]
            except (ValueError, IndexError):
                value = value.strip("\"'")
            os.environ[key] = value


load_provider_env()


def _mistral_model() -> str | None:
    value = os.environ.get("LUNA_AGENT_MISTRAL_MODEL", "").strip()
    return value or None


def _atria_provider() -> Provider | None:
    key = os.environ.get("ATRIA_API_KEY", "").strip()
    base = os.environ.get("ATRIA_BASE_URL", "").strip()
    model = os.environ.get("ATRIA_MODEL", "").strip()
    if not (key and base and model):
        return None
    return Provider("atria", "atria", model, base.rstrip("/"), "ATRIA_API_KEY", 262_144, 65_536, experimental=True)


def _registry() -> tuple[Provider, ...]:
    providers: list[Provider] = [
        Provider("inception", "inception", "mercury-2.5", "https://api.inceptionlabs.ai/v1", "INCEPTION_API_KEY", 262_144, 65_536, "high"),
        Provider("gemini", "gemini", "gemini-2.5-pro", "https://generativelanguage.googleapis.com/v1beta/openai", "GEMINI_API_KEY", 1_048_576, 65_536, "high"),
        Provider("opencode-zen", "opencode-zen", "big-pickle", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "mimo-v2.5-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "ling-3.0-flash-fin-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "nemotron-3-ultra-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "nemotron-3.5-lightning-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "muse-spark-1.3-contributor-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "laguna-s-2.1-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "ling-3.0-tiny-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "longcat-2.0-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "north-mini-code-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("opencode-zen", "opencode-zen", "deepseek-v4-flash-free", "https://opencode.ai/zen/v1", "OPENCODE_ZEN_API_KEY", 262_144, 65_536),
        Provider("cerebras", "cerebras", "gpt-oss-120b", "https://api.cerebras.ai/v1", "CEREBRAS_API_KEY", 8_192, 8_192, "high"),
        Provider("openrouter", "openrouter", "deepseek/deepseek-v4-flash-0731:free", "https://openrouter.ai/api/v1", "OPENROUTER_API_KEY", 1_310_720, 65_536, "high"),
        Provider("openrouter", "openrouter", "nvidia/nemotron-3.5-lightning:free", "https://openrouter.ai/api/v1", "OPENROUTER_API_KEY", 1_310_720, 65_536),
        Provider("openrouter", "openrouter", "inclusionai/ling-3.0-flash-fin:free", "https://openrouter.ai/api/v1", "OPENROUTER_API_KEY", 1_310_720, 65_536),
    ]
    mistral = _mistral_model()
    if mistral:
        providers.append(Provider("mistral", "mistral", mistral, "https://api.mistral.ai/v1", "MISTRAL_API_KEY", 262_144, 32_768))
    atria = _atria_provider()
    if atria:
        providers.append(atria)
    local_model = os.environ.get("LUNA_AGENT_LOCAL_MODEL", "ornith-1.5:9b").removeprefix("local/")
    ollama_host = os.environ.get("OLLAMA_HOST", "http://127.0.0.1:11434").rstrip("/")
    providers.append(Provider("ollama", "ollama", local_model, ollama_host + "/v1", "OLLAMA_API_KEY", 262_144, 32_768))
    return tuple(providers)


def _provider_order() -> list[str]:
    return [name.strip() for name in os.environ.get(
        "LUNA_AGENT_FREE_POOL",
        "inception,gemini,opencode-zen,cerebras,openrouter,mistral,atria,ollama",
    ).split(",") if name.strip()]


def build_pool() -> tuple[Provider, ...]:
    registry = _registry()
    wanted = _provider_order()
    result: list[Provider] = []
    seen: set[str] = set()
    for provider_id in wanted:
        for provider in registry:
            if provider.id != provider_id:
                continue
            key = f"{provider.id}:{provider.model}"
            if key not in seen:
                result.append(provider)
                seen.add(key)
    for provider in registry:
        key = f"{provider.id}:{provider.model}"
        if key not in seen:
            result.append(provider)
            seen.add(key)
    return tuple(result)


FREE_POOL = build_pool()


def provider_for(pool_index: int) -> Provider:
    return FREE_POOL[pool_index % len(FREE_POOL)]


def provider_key(provider: Provider) -> str | None:
    if provider.id in {"openrouter", "opencode-zen"}:
        try:
            data = json.loads(OPENROUTER_AUTH.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError, TypeError):
            data = {}
        provider_ids = ("openrouter",) if provider.id == "openrouter" else ("opencode", "opencode-zen")
        for provider_id in provider_ids:
            provider_key_data = data.get(provider_id, {}) if isinstance(data, dict) else {}
            if isinstance(provider_key_data, dict):
                key = provider_key_data.get("key")
                if isinstance(key, str) and key:
                    return key
        value = os.environ.get(provider.api_key_env, "").strip()
        return value or None
    if provider.id == "ollama":
        return "ollama"
    value = os.environ.get(provider.api_key_env, "").strip()
    return value or None


def ollama_available(provider: Provider) -> bool:
    binary = Path(os.environ.get("LUNA_AGENT_OLLAMA_BIN", str(DEFAULT_OLLAMA_BIN)))
    if not binary.is_file():
        return False
    try:
        result = subprocess.run([str(binary), "list"], text=True, capture_output=True, timeout=5)
    except (OSError, subprocess.TimeoutExpired):
        return False
    if result.returncode != 0:
        return False
    names = {line.split()[0] for line in result.stdout.splitlines()[1:] if line.split()}
    return provider.model in names


def provider_available(provider: Provider, cooldowns: dict[str, float] | None = None) -> bool:
    if cooldowns and float(cooldowns.get(provider.scope, 0)) > time.time():
        return False
    if provider.id == "ollama":
        if os.environ.get("LUNA_AGENT_ENABLE_LOCAL_FREE", "1") != "1":
            return False
        return ollama_available(provider)
    return provider_key(provider) is not None


def prepare_dsh(env: dict[str, str], dsh_home: Path, provider: Provider) -> None:
    key = provider_key(provider)
    if key is None:
        raise RuntimeError(f"credentials unavailable for provider {provider.id}")
    dsh_home.mkdir(parents=True, exist_ok=True)
    env[provider.api_key_env] = key
    settings = [
        "llm-pi-ai:",
        "  providers:",
        f"    {provider.id}:",
        f"      displayName: {provider.id}",
        "      api: openai-completions",
        f"      baseURL: {provider.base_url}",
        f"      apiKeyEnv: {provider.api_key_env}",
        "      models:",
        f"        - id: {provider.model}",
        f"          name: {provider.model}",
        f"          contextWindow: {provider.context_window}",
        f"          maxTokens: {provider.max_tokens}",
    ]
    if provider.reasoning_effort:
        settings.extend(["          reasoningEfforts:", f"            {provider.reasoning_effort}: {provider.reasoning_effort}"])
    settings.append("agent-default-model:")
    settings.append(f"  provider: {provider.id}")
    settings.append(f"  model: {provider.model}")
    if provider.reasoning_effort:
        settings.append(f"  reasoningEffort: {provider.reasoning_effort}")
    (dsh_home / "settings.yaml").write_text("\n".join(settings) + "\n", encoding="utf-8")
    env["DSH_HOME"] = str(dsh_home)
    if provider.id == "ollama":
        env["OLLAMA_API_KEY"] = "ollama"
        env["OLLAMA_HOST"] = os.environ.get("OLLAMA_HOST", "http://127.0.0.1:11434")


def provider_entries() -> list[dict[str, object]]:
    return [{
        "id": p.id,
        "scope": p.scope,
        "model": p.model,
        "available": provider_available(p),
        "experimental": p.experimental,
    } for p in FREE_POOL]
