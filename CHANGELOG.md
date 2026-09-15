# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.6.0] - 2026-09-15

### Added

- Local CLI harness providers for Claude Code (`claude_code`), Codex (`codex`),
  OpenCode (`opencode`), and Gemini CLI (`gemini_cli`), using each harness's
  existing authentication and optional default model

## [1.5.0] - 2026-07-28

### Added

- `--all` / `-a` staging of tracked modifications and deletions before generation
- Exact machine-readable output with `--stdout`, including non-rewriting `alter`
- Sequential multi-candidate generation and selection with `--generate N` / `-g N`
- Invocation-only prompt guidance with `--prompt TEXT` / `-p TEXT`
- Repository-local `prepare-commit-msg` management through `cgen hook`
- Authenticated live model discovery and searchable selection through
  `cgen model` and the configuration editor

## [1.4.1] - 2026-07-23

### Changed

- Updated all Rust dependencies, including the `ureq` 3, `toml` 1.1, `sha2` 0.11, `inquire` 0.9, `colored` 3, and `shlex` 2 major releases
- Raised the MSRV to Rust 1.85, the minimum required by the updated dependency graph
- Updated GitHub Actions to their current supported major releases

### Fixed

- Try fallback presets when a provider rejects a request as too large (`HTTP 413`)
- Include the path and line number for sensitive-content findings, and ignore the documented `your-key-here` API-key placeholder

## [1.4.0] - 2026-07-23

### Added

- Sparse project `.env` overrides with an interactive “Inherit global value” action
- Configurable diff byte/sensitive-path guards and explicit `--allow-large-diff` / `--allow-sensitive` overrides
- Linux ARM64 and x86_64 musl release artifacts with SHA-256 manifests
- Rust 1.74 MSRV declaration and CI gate
- One-shot self-correction: when the LLM returns an invalid commit message, the validator error and rejected attempt are fed back to the model for a corrective retry before failing
- `ACR_WARN_LLM_FILES_ENABLED` / `ACR_WARN_LLM_FILES_THRESHOLD`: warn on the count of files actually sent to the LLM (the token-relevant subset), merged with the staged-files warning into a single confirmation that reports the payload size
- Staged-file listing marks files excluded from the LLM payload with `(not sent to LLM)`
- `ACR_MAX_OUTPUT_TOKENS` to configure the LLM completion token cap (default 512, previously hard-coded)

### Changed

- Local `.env` updates preserve unrelated variables, comments, and absent inherited settings
- Configuration parsing rejects invalid booleans, integers, push modes, gitmoji formats, templates, locales, headers, and globs
- Provider fallbacks share one total deadline and run only for classified transient failures
- Preset/config/cache writes are owner-only, lock-protected, and atomically replaced
- History cache entries use stable identifiers, deduplicate rewrites, and retain the latest 200 commits
- Self-update targets the current installation, pins the release version, and verifies checksums before replacement
- Release CI builds with `--locked`, validates the requested tag, publishes checksums, and gates crates.io publication behind the release
- CI now enforces formatting, strict Clippy, cross-platform tests, MSRV, dependency audit, installer syntax, and aligned coverage
- Locked dependencies use patched TLS, error-handling, randomness, and test-serialization releases; terminal UI features avoid unnecessary unmaintained/yanked transitive paths while preserving Rust 1.74 support
- Rewrote the base LLM prompts around current prompt-engineering research: enumerated commit types, imperative-mood and header-length rules, anti-fabrication constraints, mode-matched few-shot examples, and explicit one-liner precedence
- Staged diffs are sent wrapped in `<diff>` tags as data to describe, with the task restated after the diff
- A blank `ACR_LLM_SYSTEM_PROMPT` now selects the built-in default prompt, and configs still storing a retired shipped default are upgraded to the current default automatically
- The config view shows `(built-in default)` for an uncustomized system prompt instead of the stored text
- Anthropic requests pin `temperature` to 0, matching the other providers
- Conventional Commit shape is enforced on LLM output only; templated and manually edited messages receive structural validation (empty/NUL)
- Staged-file warnings are evaluated after diff filtering, so hard safety gates run before any confirmation prompt
- `cgen prompt` and `cgen undo` no longer perform the startup update check

### Fixed

- Partial global TOML files no longer turn omitted default-true booleans off
- Unicode API keys/prompts no longer panic during masking or truncation
- Interpolation no longer mutates process environment or silently removes missing variables
- Confirmation cancellation can no longer select an affirmative default
- LM Studio receives the complete system instruction
- Empty, malformed, fully excluded, oversized, or sensitive diffs are stopped before provider calls
- Generated, regenerated, templated, and manually edited commit messages are validated before Git receives them
- Non-HEAD/root rewrites record the rewritten commit identity rather than HEAD
- Root-commit undo keeps changes staged
- Pushed rewrites use an explicit force-with-lease path and created tags are pushed explicitly
- Selected-repository history uses that repository for `git show`
- Concurrent preset/cache writes no longer silently overwrite one another
- The updater uses the published `auto-commit-rs` crate rather than a nonexistent package
- Non-English locales no longer instruct the LLM to translate Conventional Commit types, which produced headers that failed message validation
- Fallback presets that differ from the active configuration only by API headers are attempted instead of silently skipped

### Removed

- High-MSRV `edit` dependency; editor launching is handled internally
- Unused `enabled` flag inside the presets file's `[fallback]` section (existing files containing it still parse)

## [1.3.2] - 2026-06-25

### Added

- `--set KEY=VALUE` flag to override any setting for a single run (ephemeral, never persisted); `auto_update` excepted
- `--diff-include GLOB` / `--diff-exclude GLOB` flags to refine which files are sent to the LLM for a single run (allow-over-deny precedence)

## [1.3.1] - 2026-04-09

### Fixed

- Fixed oLLaMa streaming content issue

## [1.3.0] - 2026-04-06

### Added

- Added oLLaMa as a built-in provider

## [1.2.2] - 2026-03-05

### Fixed

- Fixed prompt issue where smaller LLMs weren't grasping single-line commits

## [1.2.1] - 2026-03-02


### Fixed

- Gitmoji spec no longer overrides the Conventional Commit spec
- When editing a LLM-generated message you start editing from the existing message


## [1.2.0] - 2026-03-02

### Added

- `ACR_DIFF_EXCLUDE_GLOBS` configuration: exclude files from LLM analysis by glob pattern while still committing them
- Default exclusion patterns for common binary/generated files: `*.json`, `*.xml`, `*.csv`, `*.pdf`, `*.lock`, images, fonts, minified assets
- Seven new built-in LLM providers: **Grok**, **DeepSeek**, **OpenRouter**, **Mistral**, **Together**, **Fireworks**, **Perplexity**
- LLM presets: save, load, rename, duplicate, delete, export/import reusable provider configurations via `cgen config`
- Fallback order: automatic retry with alternate LLM presets when the primary provider returns an HTTP error
- `ACR_FALLBACK_ENABLED` configuration flag (default: enabled) to toggle LLM fallback behavior
- Per-repository commit cache: track which commits were AI-generated
- `cgen history` subcommand to browse AI-generated commits per repository (with `git show` integration)
- `ACR_TRACK_GENERATED_COMMITS` configuration flag (default: enabled) to toggle commit tracking
- Preset management menu in `cgen config` (save current as preset, load preset, manage presets, configure fallback order)
- Preset change tracking: warns when loaded preset fields are modified and offers to update on save
- Export/import presets as TOML (with optional API key redaction)
- `cgen preset` standalone subcommand to manage LLM presets directly
- `cgen fallback` standalone subcommand to configure fallback order directly
- Config view: "Show descriptions [?]" toggle to display help text for each setting
- Config view: "Search settings [/]" to find settings by name (auto-expands matching groups)
- Config view: improved color variance with bright white for groups, bright cyan for subgroups

### Changed

- `ACR_AUTO_UPDATE` is now a global-only setting and will not be written to local `.env` files
- `call_llm` now uses `call_llm_with_fallback` internally, enabling automatic provider retry
- `generate_final_message` reports which fallback preset was used (if any)
- Config menu now includes preset and fallback management entries
- All (y/N) confirmation prompts replaced with interactive Select menus showing "Yes"/"No" options
- Config view: selected item header now strips tree-drawing characters for cleaner display
- Preset management: restructured menu - select a preset first via "Manage existing preset...", then choose action (Rename/Duplicate/Delete)

### Fixed

- Cursor no longer resets to top of view when collapsing headers on the `cgen config` view

## [1.1.0] - 2026-02-24

### Added

- `cgen update` subcommand to manually update to the latest version
- `ACR_AUTO_UPDATE` configuration flag (defaults to unset; prompts on first run)
- Automatic version checking against GitHub releases on every run
- Auto-update support when `ACR_AUTO_UPDATE=1` (updates silently before proceeding)
- Update warning displayed at the end of output when a newer version is available and auto-update is off
- `cgen prompt` subcommand to print the LLM system prompt without running anything
- `cgen config` now auto-detects git repo: prompts for global vs local scope inside a repo, opens global directly outside one

### Changed

- Staged files display now uses tree-style characters (`├──`, `└──`) instead of bullet points
- Boolean config fields display "enabled"/"disabled" instead of "1 (yes)"/"0 (no)" in the interactive config UI
- Interactive config groups settings into collapsible tree sections (Basic expanded, Advanced collapsed with subgroups)
- `cgen config --global` flag removed; scope selection is now interactive when inside a git repo

## [1.0.0] - 2026-02-23

- Initial release of the tool
