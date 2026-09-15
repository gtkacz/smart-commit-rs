# Quick Start

## 1. Get an API key

cgen defaults to [Groq](https://console.groq.com/keys), which has a free tier.
Any other [built-in provider](../providers/built-in-providers.md) works the same
way — and local providers (Ollama, LM Studio, Claude Code, Codex, OpenCode, and
Gemini CLI) need no `ACR_API_KEY`.

## 2. Configure

Either run the interactive editor and set **API Key** (and optionally
**Provider**/**Model**) under the *Basic* group:

```sh
cgen config
```

or export the key in your shell:

```sh
export ACR_API_KEY=your-key-here
```

`cgen config` saves to the global config file, so this is a one-time step. See
[Config File & Locations](../configuration/config-file-and-locations.md) for
per-repository overrides.

## 3. Stage and generate

```sh
git add .
cgen
```

cgen lists the staged files (marking any that are
[excluded from the LLM payload](../configuration/diff-exclusion-patterns.md)),
sends the filtered diff to your provider, and shows the proposed message with a
review menu:

- **Accept** — create the commit with this message
- **Regenerate** — ask the LLM for a new message
- **Edit** — open the message in your editor before committing
- **Cancel** — abort; nothing is committed

After the commit, cgen asks whether to push (change this with
`ACR_POST_COMMIT_PUSH=never|ask|always`). On first run it also asks once
whether to enable [automatic updates](../configuration/updating.md).

Not ready to commit? `cgen --dry-run` prints the generated message without
creating a commit.

## Next steps

- [Usage](../usage.md) — all commands and flags
- [Configuration](../configuration/config-file-and-locations.md) — every setting, message language, gitmoji, templates
- [Presets](../providers/presets.md) — save and switch between provider setups
