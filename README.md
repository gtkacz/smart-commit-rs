# smart-commit-rs

[![crates.io](https://img.shields.io/crates/v/auto-commit-rs)](https://crates.io/crates/auto-commit-rs)
[![CI](https://github.com/gtkacz/smart-commit-rs/actions/workflows/test.yml/badge.svg)](https://github.com/gtkacz/smart-commit-rs/actions/workflows/test.yml)
[![docs](https://img.shields.io/badge/docs-mdBook-blue)](https://gtkacz.github.io/smart-commit-rs/)
[![license](https://img.shields.io/badge/license-MIT-green)](LICENSE)

**cgen** generates git commit messages from your staged diff using an LLM, then
lets you review, edit, or regenerate the message before anything is committed.
It ships as a single ~3 MB executable with no language runtime to install. The
executable is `cgen`; the crates.io package is `auto-commit-rs`.

## Features

- **17 built-in providers** — cloud APIs, local model servers, and the Claude
  Code, Codex, OpenCode, and Gemini CLI harnesses — plus any OpenAI-compatible
  custom endpoint. CLI harnesses use their existing login; local model servers
  need no API key.
- **Review before committing** — accept, regenerate, edit in your editor, or cancel.
- **Flexible generation** — stage tracked changes with `--all`, print a clean
  message with `--stdout`, request multiple candidates, or add per-run guidance.
- **Git hook integration** — generate messages automatically from
  `prepare-commit-msg` while preserving and chaining an existing hook.
- **Live model discovery** — search provider model catalogs with `cgen model`
  or from the configuration editor.
- **Safety guards** — sensitive files and high-confidence secret patterns are
  blocked from the LLM payload; oversized diffs and large stagings require
  explicit confirmation.
- **Conventional Commits by default** — optional gitmoji, custom templates, and
  commit messages in any language via `ACR_LOCALE`.
- **Presets & fallback** — save provider configurations and fall back through
  them automatically when the primary LLM fails.
- **History tools** — regenerate past commit messages (`cgen alter`), undo the
  latest commit safely (`cgen undo`), browse AI-generated commits (`cgen history`).
- **Self-updating** — checksum-verified updates via `cgen update` or opt-in auto-update.

## Why Rust?

Tools like [opencommit](https://github.com/di-sukharev/opencommit) do the same thing but require Node.js and weigh in at **~100MB** of `node_modules`. cgen is a roughly **3MB** self-contained executable. GNU/Linux release builds use the platform C library; a musl artifact is also published for portable x86_64 Linux installs.

| | cgen | opencommit |
|---|---|---|
| Install size | ~2 MB | ~100 MB |
| Runtime deps | None | Node.js |
| Startup time | Instant | ~300ms (Node cold start) |
| Generation time | ~800ms | ~4s |
| Distribution | Single binary | npm install |

## Install

```sh
# Linux / macOS
curl -fsSL https://raw.githubusercontent.com/gtkacz/smart-commit-rs/main/scripts/install.sh | bash

# Windows (PowerShell)
irm https://raw.githubusercontent.com/gtkacz/smart-commit-rs/main/scripts/install.ps1 | iex

# Cargo
cargo install auto-commit-rs
```

→ [Full installation instructions](https://gtkacz.github.io/smart-commit-rs/getting-started/installation.html) (manual download, custom install directory, binaries per platform).

## Quick Start

```sh
# 1. Set your API key (one-time)
cgen config
# or: export ACR_API_KEY=your-key-here

# 2. Stage files and generate commit
git add .
cgen
```

→ [Quick start walkthrough](https://gtkacz.github.io/smart-commit-rs/getting-started/quick-start.html) (what each step does, the review menu, push behavior).

## Usage

```
cgen                    # Generate commit message and commit
cgen --dry-run          # Generate and show message without committing
cgen --all              # Stage tracked modifications/deletions, then generate
cgen --stdout           # Print only the generated message (for scripts)
cgen --generate 3       # Generate and choose from three candidates
cgen --prompt "focus on the API compatibility impact"
cgen --tag              # Also create the next semantic version tag
cgen config             # Interactive config editor (auto-detects scope)
cgen model              # Discover and select a provider model
cgen hook install       # Install repository-local Git hook integration
cgen alter <hash>       # Regenerate message from that commit's diff and rewrite it
cgen undo               # Undo latest commit with safety prompts (soft reset)
cgen history            # Browse AI-generated commits for the current repo
```

Any arguments passed to `cgen` (without a subcommand) are forwarded directly to `git commit`.

→ [Full command reference](https://gtkacz.github.io/smart-commit-rs/usage.html) (all flags, per-run overrides, diff filters, remaining subcommands).

## Configuration

All settings use the `ACR_` prefix and resolve in layers: defaults → global
TOML → local `.env` in the repo root → process environment → CLI `--set`
(highest priority, this run only). The settings you'll touch most:

| Variable | Default | Description |
|----------|---------|-------------|
| `ACR_PROVIDER` | `groq` | LLM provider (see [Providers](#providers)) |
| `ACR_MODEL` | `llama-3.3-70b-versatile` | Model name |
| `ACR_API_KEY` | unset | API key (required by cloud providers) |
| `ACR_LOCALE` | `en` | Commit message language |
| `ACR_ONE_LINER` | `1` | Single-line commits (`1`/`0`) |
| `ACR_USE_GITMOJI` | `0` | Prepend gitmoji to messages (`1`/`0`) |
| `ACR_REVIEW_COMMIT` | `1` | Review message before committing (`1`/`0`) |
| `ACR_POST_COMMIT_PUSH` | `ask` | Push after commit (`never`/`ask`/`always`) |

→ [Full settings reference](https://gtkacz.github.io/smart-commit-rs/configuration/config-file-and-locations.html) covers every variable, plus [per-invocation overrides](https://gtkacz.github.io/smart-commit-rs/configuration/per-invocation-overrides.html), [variable interpolation](https://gtkacz.github.io/smart-commit-rs/configuration/variable-interpolation.html), [diff exclusion patterns](https://gtkacz.github.io/smart-commit-rs/configuration/diff-exclusion-patterns.html), [safety & workflow controls](https://gtkacz.github.io/smart-commit-rs/configuration/safety-and-workflow-controls.html), and [updating](https://gtkacz.github.io/smart-commit-rs/configuration/updating.html).

## Providers

Built-in providers: **Groq** (default), **OpenAI**, **Anthropic**, **Gemini**, **Grok**, **DeepSeek**, **OpenRouter**, **Mistral**, **Together**, **Fireworks**, **Perplexity**, **LM Studio**, **Ollama**, **Claude Code**, **Codex**, **OpenCode**, and **Gemini CLI** — plus any OpenAI-compatible custom endpoint.

→ [Provider list and default models](https://gtkacz.github.io/smart-commit-rs/providers/built-in-providers.html), [presets](https://gtkacz.github.io/smart-commit-rs/providers/presets.html), and [fallback order](https://gtkacz.github.io/smart-commit-rs/providers/fallback-order.html).

## Documentation

The full documentation lives at **[gtkacz.github.io/smart-commit-rs](https://gtkacz.github.io/smart-commit-rs/)** — installation, the complete configuration and command reference, provider details, and internals such as [prompt design](https://gtkacz.github.io/smart-commit-rs/internals/prompt-design.html) and [commit history tracking](https://gtkacz.github.io/smart-commit-rs/internals/commit-history.html).

## AI-Generated Code Disclaimer

The majority of the code in this repository was generated by agentic AI. Every
pull request and architecture decision is reviewed and refined by a human
developer, and the codebase is gated by comprehensive unit tests and CI
(coverage, formatting, lints, dependency audit, tests on Linux/macOS/Windows).
The software is nevertheless provided "as is", without warranty of any kind.

## Contributing

Contributions are welcome! Whether it's a new provider (often just 5 lines), a bug fix, or a documentation improvement, every bit helps.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development setup, the quality gates CI enforces, and a step-by-step guide to adding a new default provider.

## License

[MIT](LICENSE)
