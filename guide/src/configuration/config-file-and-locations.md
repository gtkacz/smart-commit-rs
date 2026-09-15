# Config File & Locations

All settings use the `ACR_` prefix. Layered resolution is defaults → global
TOML → local `.env` → process environment → CLI `--set` (highest priority,
this run only). A project `.env` is a sparse overlay: it may contain only the
keys that project overrides, and every absent key continues to inherit from
global config/defaults. Saving local config preserves unrelated variables and
comments; choosing "Inherit global value" removes that local assignment.

`ACR_AUTO_UPDATE` is the one global-only setting: it is never written to local
`.env` files and cannot be overridden with `--set`.

## Settings Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `ACR_PROVIDER` | `groq` | LLM provider (`groq`, `openai`, `anthropic`, `gemini`, `grok`, `deepseek`, `openrouter`, `mistral`, `together`, `fireworks`, `perplexity`, `lm_studio`, `ollama`, `claude_code`, `codex`, `opencode`, `gemini_cli`, or custom) |
| `ACR_MODEL` | `llama-3.3-70b-versatile` | Model name |
| `ACR_API_KEY` | unset | API key (required by cloud providers) |
| `ACR_API_URL` | auto | API endpoint (auto-resolved from provider) |
| `ACR_API_HEADERS` | auto | Custom headers (`Key: Value, Key2: Value2`) |
| `ACR_LOCALE` | `en` | Commit message language |
| `ACR_ONE_LINER` | `1` | Single-line commits (`1`/`0`) |
| `ACR_COMMIT_TEMPLATE` | `$msg` | Template, `$msg` is replaced with LLM output |
| `ACR_LLM_SYSTEM_PROMPT` | (built-in) | Base system prompt |
| `ACR_USE_GITMOJI` | `0` | Enable gitmoji (`1`/`0`) |
| `ACR_GITMOJI_FORMAT` | `unicode` | Gitmoji style (`unicode`/`shortcode`) |
| `ACR_REVIEW_COMMIT` | `1` | Review message before committing (`1`/`0`) |
| `ACR_POST_COMMIT_PUSH` | `ask` | Post-commit push behavior (`never`/`ask`/`always`) |
| `ACR_SUPPRESS_TOOL_OUTPUT` | `0` | Suppress git subprocess output (`1`/`0`) |
| `ACR_WARN_STAGED_FILES_ENABLED` | `1` | Warn when staged file count exceeds threshold (`1`/`0`) |
| `ACR_WARN_STAGED_FILES_THRESHOLD` | `20` | Staged files warning threshold (warn when count is greater) |
| `ACR_WARN_LLM_FILES_ENABLED` | `1` | Warn when the count of files sent to the LLM exceeds threshold (`1`/`0`) |
| `ACR_WARN_LLM_FILES_THRESHOLD` | `20` | LLM-analyzed files warning threshold (warn when count is greater) |
| `ACR_CONFIRM_NEW_VERSION` | `1` | Ask before creating the computed `--tag` version (`1`/`0`) |
| `ACR_AUTO_UPDATE` | unset | Enable automatic updates (`1`/`0`); prompts on first run if unset |
| `ACR_FALLBACK_ENABLED` | `1` | Try fallback presets when primary LLM fails (`1`/`0`) |
| `ACR_TRACK_GENERATED_COMMITS` | `1` | Track AI-generated commits per repository (`1`/`0`) |
| `ACR_DIFF_EXCLUDE_GLOBS` | (see [Diff Exclusion Patterns](diff-exclusion-patterns.md)) | Comma-separated glob patterns for files to exclude from LLM analysis |
| `ACR_MAX_DIFF_BYTES` | `200000` | Maximum filtered diff bytes accepted without `--allow-large-diff` |
| `ACR_MAX_OUTPUT_TOKENS` | `512` | Maximum tokens the LLM may generate for one commit message |
| `ACR_SENSITIVE_FILE_GLOBS` | `.env,.env.*,*.pem,*.key,id_rsa,id_ed25519,*credentials*,*secrets*` | Paths that require `--allow-sensitive` before LLM analysis |

## Config Locations

- **Global**: `~/.config/cgen/config.toml` (Linux), `~/Library/Application Support/cgen/config.toml` (macOS), `%APPDATA%\cgen\config.toml` (Windows)
- **Local**: `.env` in git repo root

## The `cgen config` editor

`cgen config` edits these files interactively. Inside a git repo it asks
whether to edit local or global settings; outside a repo it opens the global
config directly. The menu groups settings, supports **Show descriptions [?]**
(inline help for each setting) and **Search settings [/]** (auto-expands
matching groups), and includes entries for managing
[presets](../providers/presets.md) and
[fallback order](../providers/fallback-order.md). Changing the provider
automatically resets the model to that provider's default.
