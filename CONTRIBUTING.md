# Contributing to cgen

First off, thank you for considering contributing to cgen! Every contribution helps — whether it's a bug report, a new provider, documentation improvement, or a feature implementation.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Adding a New Default Provider](#adding-a-new-default-provider)
- [Making Changes](#making-changes)
- [Pull Request Process](#pull-request-process)
- [Code Style](#code-style)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Features](#suggesting-features)

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```sh
   git clone https://github.com/YOUR_USERNAME/smart-commit-rs.git
   cd smart-commit-rs
   ```
3. **Add the upstream remote**:
   ```sh
   git remote add upstream https://github.com/gtkacz/smart-commit-rs.git
   ```

## Development Setup

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain, 1.70+)
- Git

### Build & Run

```sh
# Debug build (fast compilation, slower binary)
cargo build

# Run directly
cargo run

# Run with arguments
cargo run -- config

# Release build (slow compilation, optimized binary)
cargo build --release

# Run tests
cargo test

# Check for warnings without building
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```

### Testing Locally

To test the full flow you need a valid API key for at least one provider:

```sh
# Set a key for testing
export ACR_API_KEY=your-test-key

# Stage some changes and run
git add some_file
cargo run
```

To test the interactive config menu:

```sh
cargo run -- config          # Inside a repo: choose local (.env) or global (TOML)
cargo run -- config          # Outside a repo: opens global TOML directly
```

To test new commit workflow controls:

```sh
# Dry run (should NOT create a commit)
cargo run -- --dry-run

# Undo latest commit (soft reset, keeps changes staged)
cargo run -- undo
```

Suggested manual smoke checks:
- `ACR_POST_COMMIT_PUSH=never|ask|always` behavior after commit creation
- `ACR_WARN_STAGED_FILES_ENABLED=1` + low `ACR_WARN_STAGED_FILES_THRESHOLD` to trigger confirmation
- `ACR_SUPPRESS_TOOL_OUTPUT=1` to confirm git output is hidden
- `undo` on unpushed commit
- `undo` on pushed commit (should warn and require confirmation)

Automated quality gates (same checks as CI):

```sh
# Full test suite
cargo test --locked

# Coverage gate for core logic
cargo llvm-cov --locked --lib --tests \
  --ignore-filename-regex 'src/main.rs|src/cli.rs' \
  --summary-only \
  --fail-under-lines 95
```

## Project Structure

```
src/
├── main.rs              # Entry point, CLI dispatch, main flow
├── cli.rs               # clap derive definitions + interactive config menu (inquire)
├── config.rs            # AppConfig struct, layered resolution, TOML/env I/O
├── provider.rs          # Provider registry, API adapters, HTTP call, response parsing
├── prompt.rs            # System prompt assembly from config flags
├── git.rs               # git diff, git commit via std::process::Command
└── interpolation.rs     # $VAR template engine for URL/headers

data/
└── conventions/         # Reference specs for conventional commits & gitmoji
    ├── CONVENTIONAL_COMMIT.md
    └── GITMOJI.md

.github/workflows/
└── release.yml          # Cross-platform CI/CD (builds on tag push)
```

**Design principles**:
- One file = one concern. No nested module directories.
- Synchronous only — no async runtime (`ureq` instead of `reqwest`+`tokio`).
- Minimal dependencies — every crate must justify its inclusion by binary size or maintenance burden.
- `std::process::Command` for git operations — no `git2`/`gitoxide` for 3 shell commands.

## Adding a New Default Provider

This is one of the easiest and most valuable ways to contribute. A default provider means users can just set `ACR_PROVIDER=provider_name` and `ACR_API_KEY=...` without needing to configure the URL or headers manually.

### Step-by-step

1. **Open `src/provider.rs`** and find the `get_provider()` function.

2. **Add a new match arm** with the provider's API details:

   ```rust
   "your_provider" => Some(ProviderDef {
       api_url: "https://api.example.com/v1/chat/completions",
       api_headers: "Authorization: Bearer $ACR_API_KEY",
       default_model: "your-model",
       format: RequestFormat::OpenAiCompat, // or Gemini, Anthropic, LmStudio
       response_path: "choices.0.message.content",
   }),
   ```

3. **Choose the right `RequestFormat`**:
   - `OpenAiCompat` — Most providers use this (OpenAI-compatible chat completions). Request body: `{ model, messages: [{role, content}], max_tokens, temperature }`.
   - `Gemini` — Google's format with `system_instruction` and `contents` arrays.
   - `Anthropic` — Similar to OpenAI but with `system` as a top-level string field.
   - `LmStudio` — LM Studio chat endpoint format. Request body: `{ model, input }`.

   If the provider uses a completely different format, you may need to add a new variant to `RequestFormat` and a matching arm in `build_request_body()`.

4. **Set `response_path`** — this is a dot-separated path to the generated text in the JSON response. For example:
   - OpenAI-compatible: `choices.0.message.content`
   - Gemini: `candidates.0.content.parts.0.text`
   - LM Studio: `output` (the parser selects the item where `type == "message"` and returns its `content`)
   - Use numbers for array indices: `results.0.text`

5. **URL/header interpolation** — you can use `$ACR_API_KEY`, `$ACR_MODEL`, or any environment variable in the `api_url` and `api_headers` strings. They get expanded at runtime.

6. **Update `src/cli.rs`** — add the provider name to the `choices` list in the `"PROVIDER"` match arm of `interactive_config()`:

   ```rust
   "PROVIDER" => {
       let choices = vec!["gemini", "openai", "anthropic", "your_provider", "(custom)"];
       // ...
   }
   ```

7. **Update the README** — add the provider to the "Built-in providers" line in the Providers section.

8. **Test it** — if you have access to the provider's API, verify the full flow works. If not, mention this in your PR and someone will test it before merging.

### Example: Adding Mistral

```rust
"mistral" => Some(ProviderDef {
    api_url: "https://api.mistral.ai/v1/chat/completions",
    api_headers: "Authorization: Bearer $ACR_API_KEY",
    default_model: "mistral-small-latest",
    format: RequestFormat::OpenAiCompat,
    response_path: "choices.0.message.content",
}),
```

### Example: Adding LM Studio

```rust
"lm_studio" => Some(ProviderDef {
    api_url: "http://localhost:1234/api/v1/chat",
    api_headers: "Content-Type: application/json",
    default_model: "qwen/qwen3.5-35b-a3b",
    format: RequestFormat::LmStudio,
    response_path: "output",
}),
```

That's it — most OpenAI-compatible providers are short additions, while custom payload APIs (like LM Studio) need a dedicated request/response format branch.

## Making Changes

1. **Create a feature branch** from `main`:
   ```sh
   git checkout main
   git pull upstream main
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** — keep commits focused and atomic.

3. **Ensure quality**:
   ```sh
   cargo fmt       # Format code
   cargo clippy    # Lint — fix all warnings
   cargo test      # Run tests
   cargo build     # Verify it compiles
   ```

4. **Commit** with a descriptive message following [Conventional Commits](https://www.conventionalcommits.org/) or just use `cgen` ;) :
   ```
   feat(provider): add Mistral as default provider
   fix(config): handle missing .env gracefully
   docs: add Mistral to provider list in README
   ```

## Pull Request Process

1. **Push** your branch to your fork:
   ```sh
   git push origin feature/your-feature-name
   ```

2. **Open a Pull Request** against `main` on the upstream repository.

3. **In the PR description**, include:
   - What the change does and why
   - How to test it (if applicable)
   - Screenshots for UI changes (config menu, spinner, etc.)

4. **Keep it small** — one concern per PR. A provider addition + a bug fix should be two separate PRs.

5. **Be responsive** — if changes are requested, push follow-up commits to the same branch.

### What makes a good PR

- Follows the existing code patterns and style
- Doesn't introduce new dependencies without justification
- Keeps the binary small (check `cargo build --release` size)
- Includes documentation updates when behavior changes
- Has a clear, concise title using conventional commit format

## Code Style

- **Format**: Always run `cargo fmt` before committing.
- **Lints**: Fix all `cargo clippy` warnings.
- **Error handling**: Use `anyhow` for errors. Use `.context("description")` to add context to errors. Use `bail!()` for early returns.
- **No `unwrap()` in production code** — use `?` or `.context()` instead. `unwrap()` is acceptable only in cases where failure is truly impossible (e.g., compiling a hardcoded regex).
- **Dependencies**: Prefer crates that are lightweight and well-maintained. Always consider binary size impact. If a feature can be done in 20 lines of code, don't add a crate for it.
- **Comments**: Only where the "why" isn't obvious from the code. No doc comments on private internals unless they're complex.

## Reporting Bugs

Open an [issue](https://github.com/gtkacz/smart-commit-rs/issues/new) with:

- **cgen version** (`cgen --version`)
- **OS and architecture** (e.g., Windows 11 x64, macOS ARM)
- **What you expected** vs **what happened**
- **Steps to reproduce** — the minimum commands to trigger the bug
- **Error output** — full terminal output including the error message

## Suggesting Features

Open an [issue](https://github.com/gtkacz/smart-commit-rs/issues/new) with:

- **What problem it solves** — describe the use case, not just the solution
- **Proposed behavior** — how it would work from the user's perspective
- **Alternatives considered** — other ways you thought about solving it

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
