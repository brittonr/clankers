# Nickel Configuration

Clankers supports [Nickel](https://nickel-lang.org) as an alternative to JSON for settings files. Nickel gives you comments, typed contracts, computed values, and deep merge — things JSON can't do.

## Quick Start

Generate a starter config:

```bash
clankers config init --nickel          # project config
clankers config init --nickel --global # global config
```

This creates a `settings.ncl` with the contract import and commented fields.

## Writing settings.ncl

A minimal config that overrides the default model:

```nickel
(import "clankers://settings") & {
  model = "claude-opus-4-6",
}
```

The `import "clankers://settings"` brings in the built-in contract which provides type checking and defaults. You only specify fields you want to change.

A more complete example:

```nickel
(import "clankers://settings") & {
  model = "claude-opus-4-6",
  maxTokens = 32768,
  planMode = true,

  keymap = {
    preset = "vim",
  },

  hooks = {
    disabledHooks = ["pre-tool"],
  },

  memory = {
    globalCharLimit = 4400,
  },
}
```

You can also write plain records without the contract import — they work fine, you just don't get type validation or defaults:

```nickel
{
  model = "claude-opus-4-6",
  maxTokens = 32768,
}
```

## File Precedence

At each config layer (global, project), clankers checks for `settings.ncl` first, then falls back to `settings.json`. You can mix formats across layers — a global `.ncl` with a project `.json` works fine.

## CLI Commands

```bash
clankers config check           # validate all config layers
clankers config export          # print merged settings as JSON
clankers config export --global # print global-only settings
clankers config paths           # show config file locations
```

## Deep Merge

Both JSON and Nickel configs benefit from deep merge. When a project config overrides a nested field, other fields in that object are preserved:

```json
// global: {"hooks": {"enabled": true, "scriptTimeoutSecs": 10}}
// project: {"hooks": {"disabledHooks": ["pre-tool"]}}
// result: hooks.enabled=true, hooks.scriptTimeoutSecs=10, hooks.disabledHooks=["pre-tool"]
```

Arrays are replaced wholesale — not concatenated.

## Building Without Nickel

The Nickel evaluator is behind the `nickel` cargo feature. Disable it for smaller builds:

```toml
clankers-config = { version = "0.1.0", default-features = false }
```

When disabled, `.ncl` files are ignored — only `.json` loading is available.
