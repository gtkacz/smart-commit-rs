# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- 

### Changed

- 

### Fixed

-

### Removed

-

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
