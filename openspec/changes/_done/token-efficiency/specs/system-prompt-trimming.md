# Spec: System Prompt Trimming

## Overview

Make the system prompt conditional. Move Nix, model switching, HEARTBEAT, and
process monitoring sections behind feature detection so they're only included
when relevant. Drops base prompt from ~799 to ~400 tokens.

## Current State

`default_system_prompt()` in `crates/clankers-agent/src/system_prompt.rs`
returns a single `&'static str` with five sections concatenated:

1. **Base role + guidelines** (~200 tokens) — always relevant
2. **Nix package management** (~180 tokens) — relevant when `nix` is available
3. **Model switching** (~120 tokens) — relevant when multi-model configured
4. **HEARTBEAT.md** (~80 tokens) — relevant in daemon mode only
5. **Process monitoring** (~80 tokens) — relevant when procmon tool active

Total: ~660 content tokens + ~140 formatting = ~799 tokens.

## Target State

Replace the monolithic static string with a builder that conditionally
appends sections:

```rust
// crates/clankers-agent/src/system_prompt.rs

/// Feature flags that control which system prompt sections are included.
#[derive(Debug, Clone, Default)]
pub struct PromptFeatures {
    /// Nix is available on this system (`which nix` succeeded at startup).
    pub nix_available: bool,
    /// Multiple models/roles are configured (model switching makes sense).
    pub multi_model: bool,
    /// Running in daemon or RPC mode (HEARTBEAT.md is relevant).
    pub daemon_mode: bool,
    /// Process monitor is active (procmon tool is in an active tier).
    pub process_monitor: bool,
}
```

### Section Constants

Break the monolithic string into named constants:

```rust
const BASE_PROMPT: &str = r#"You are clankers, a terminal coding agent. You help users by reading files, executing commands, editing code, and writing new files.

Guidelines:
- Use tools to explore the codebase before making changes
- Read files before editing them to understand context
- Make precise, surgical edits rather than full file rewrites
- Run tests after making changes to verify correctness
- Be concise in responses
- Show file paths clearly when discussing files"#;

const NIX_SECTION: &str = r#"
## Handling Missing Commands/Packages

When a command is not found, use Nix to run it ephemerally. **NEVER use `nix profile install`**.

```bash
nix-shell -p <package> --run "<command>"
```"#;

const MODEL_SWITCHING_SECTION: &str = r#"
## Model Switching

You have a `switch_model` tool. Use it when:
- The task is simpler than expected — switch to 'smol' for speed/cost savings.
- The task is harder than expected — switch to 'slow' for maximum capability.
- You've finished a hard sub-task — switch back to 'smol' or 'default'.

Don't switch unnecessarily. Stay on the current model if it's appropriate."#;

const HEARTBEAT_SECTION: &str = r#"
## HEARTBEAT.md (daemon mode)

You have a HEARTBEAT.md in your session directory. A background scheduler
reads it periodically. Use it for reminders and recurring tasks."#;

const PROCMON_SECTION: &str = r#"
## Process Monitoring

You have a `procmon` tool to inspect child processes. Actions: list, summary,
inspect (by PID), history."#;
```

### Builder Function

```rust
/// Build the default system prompt with only relevant sections included.
pub fn default_system_prompt(features: &PromptFeatures) -> String {
    let mut parts = vec![BASE_PROMPT.to_string()];

    if features.nix_available {
        parts.push(NIX_SECTION.to_string());
    }
    if features.multi_model {
        parts.push(MODEL_SWITCHING_SECTION.to_string());
    }
    if features.daemon_mode {
        parts.push(HEARTBEAT_SECTION.to_string());
    }
    if features.process_monitor {
        parts.push(PROCMON_SECTION.to_string());
    }

    parts.join("\n")
}
```

## Feature Detection

### `nix_available`

Checked once at startup:

```rust
fn detect_nix() -> bool {
    std::process::Command::new("which")
        .arg("nix")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

Cache the result in `CommandContext` or `AppConfig` — no need to re-check.

### `multi_model`

True when settings contain model role overrides or more than one provider is
configured. Already known at settings load time.

### `daemon_mode`

Set by the daemon/rpc command path. The caller knows this when constructing
the agent.

### `process_monitor`

True when the orchestration tier is active (ties into tiered tools spec).
If tiered tools aren't implemented yet, default to true in interactive mode
and false in headless mode.

## Trimmed Section Content

The Nix section is the most aggressively trimmed. The current version includes
six example code blocks (~180 tokens). The trimmed version keeps one example
(~60 tokens). The full examples aren't needed — the model already knows nix-shell
syntax. A single example is enough to establish the "ephemeral, never install"
pattern.

The model switching section drops the bullet-point examples and keeps the
one-sentence rule. The model knows what "simpler" and "harder" mean.

## Integration with `assemble_system_prompt()`

`assemble_system_prompt()` already accepts a `base_prompt: &str` parameter.
The change is at the call site:

```rust
// Before:
let base = default_system_prompt();

// After:
let features = PromptFeatures {
    nix_available: ctx.nix_available,
    multi_model: settings.has_model_roles(),
    daemon_mode: false,
    process_monitor: tool_set.is_active(ToolTier::Orchestration),
};
let base = default_system_prompt(&features);
```

Everything downstream (`assemble_system_prompt`, SYSTEM.md override,
APPEND_SYSTEM.md, AGENTS.md, context files) works unchanged.

## Token Savings by Scenario

| Scenario | Sections included | Approx tokens |
|----------|------------------|-------------:|
| Headless (`-p`) on NixOS | Base + Nix | ~260 |
| Headless (`-p`) on non-Nix | Base only | ~200 |
| Interactive on NixOS | Base + Nix + Model | ~380 |
| Interactive on non-Nix | Base + Model | ~320 |
| Daemon on NixOS | All sections | ~660 |

Savings vs current 799-token prompt: 140–600 tokens per turn.

## Invariants

- `SYSTEM.md` override still replaces the entire base prompt (including
  conditional sections). If a user provides SYSTEM.md, none of the
  conditional logic applies.
- `APPEND_SYSTEM.md` still appends after whatever base is used.
- The base prompt (role + guidelines) is never omitted. There is no feature
  flag for it.
- Trimmed section content must still convey the same behavioral rules as
  the current verbose versions. The model must know to use `nix-shell`
  instead of `nix profile install`, to use `switch_model` appropriately, etc.

## Tests

- `prompt_headless_no_nix`: only base section, ~200 tokens.
- `prompt_headless_nix`: base + nix, no model switching.
- `prompt_interactive`: base + nix + model, no heartbeat/procmon.
- `prompt_daemon`: all sections present.
- `prompt_system_md_overrides`: SYSTEM.md replaces everything regardless of features.
- `detect_nix_caches`: nix detection runs once, not per-prompt.
