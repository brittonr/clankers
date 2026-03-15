# Spec: Tiered Tool Registration

## Overview

Replace the flat `Vec<Arc<dyn Tool>>` returned by `build_tools_with_env()` with
a `ToolSet` that groups tools into tiers. Only active tiers are sent to the API.
Tiers can be activated/deactivated mid-session.

## Current State

`src/modes/common.rs` line 43: `build_tools_with_env()` returns 25 tools
unconditionally. All 25 tool schemas are serialized into every API request.

Tool schemas are sent as JSON in the `tools` array of the API request body.
Each tool definition includes name, description, and input_schema (JSON Schema).
Complex tools like `subagent` have nested object/array schemas that cost
hundreds of tokens.

## Tier Assignments

### Tier 0: Core (always active)

```
read        — read file contents
write       — create/overwrite files
edit        — surgical find-and-replace edits
bash        — execute shell commands
grep        — gitignore-aware search (ripgrep)
find        — gitignore-aware file finding
ls          — directory listing
```

7 tools, ~2,500 schema tokens.

### Tier 1: Orchestration (on demand)

```
subagent           — spawn ephemeral subagents
delegate_task      — delegate to persistent workers
signal_loop_success — break out of loops
procmon            — inspect child processes
```

4 tools, ~1,800 schema tokens. The `subagent` tool alone is ~435 tokens.

### Tier 2: Specialty (interactive default)

```
nix         — run nix commands
web         — fetch URLs
commit      — git commit
review      — code review
ask         — ask user for input
image_gen   — generate images
todo        — manage todo items
switch_model — change model mid-session
```

8 tools, ~1,800 schema tokens.

### Tier 3: Matrix (daemon only)

```
matrix_send   — send Matrix messages
matrix_read   — read Matrix messages
matrix_rooms  — list Matrix rooms
matrix_peers  — list Matrix peers
matrix_join   — join Matrix rooms
matrix_rpc    — RPC over Matrix
```

6 tools, ~1,200 schema tokens.

## Data Structures

```rust
// src/modes/common.rs

use std::collections::HashSet;
use std::sync::Arc;

use crate::tools::Tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTier {
    Core,
    Orchestration,
    Specialty,
    Matrix,
}

pub struct ToolSet {
    /// All tools with their tier assignment.
    all: Vec<(ToolTier, Arc<dyn Tool>)>,
    /// Currently active tiers.
    active: HashSet<ToolTier>,
}

impl ToolSet {
    pub fn new(env: &ToolEnv, tiers: impl IntoIterator<Item = ToolTier>) -> Self {
        let active: HashSet<ToolTier> = tiers.into_iter().collect();
        let all = build_all_tools(env);
        Self { all, active }
    }

    /// Tools to send to the API on the current turn.
    pub fn active_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.all.iter()
            .filter(|(tier, _)| self.active.contains(tier))
            .map(|(_, tool)| tool.clone())
            .collect()
    }

    /// All tools regardless of tier (for /tools list, collision detection).
    pub fn all_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.all.iter().map(|(_, tool)| tool.clone()).collect()
    }

    pub fn activate(&mut self, tier: ToolTier) {
        self.active.insert(tier);
    }

    pub fn deactivate(&mut self, tier: ToolTier) {
        self.active.remove(&tier);
    }

    pub fn is_active(&self, tier: ToolTier) -> bool {
        self.active.contains(&tier)
    }
}
```

## Tool Assignment Function

```rust
fn build_all_tools(env: &ToolEnv) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    // ... (same tool construction as today, but each wrapped with its tier)
    vec![
        // Core
        (ToolTier::Core, Arc::new(ReadTool::new())),
        (ToolTier::Core, Arc::new(WriteTool::new())),
        (ToolTier::Core, Arc::new(EditTool::new())),
        (ToolTier::Core, Arc::new(bash_tool)),
        (ToolTier::Core, Arc::new(GrepTool::new())),
        (ToolTier::Core, Arc::new(FindTool::new())),
        (ToolTier::Core, Arc::new(LsTool::new())),
        // Orchestration
        (ToolTier::Orchestration, Arc::new(subagent_tool)),
        (ToolTier::Orchestration, Arc::new(delegate_tool)),
        (ToolTier::Orchestration, Arc::new(SignalLoopTool::new())),
        (ToolTier::Orchestration, Arc::new(procmon_tool)),
        // Specialty
        (ToolTier::Specialty, Arc::new(NixTool::new())),
        (ToolTier::Specialty, Arc::new(WebTool::new())),
        (ToolTier::Specialty, Arc::new(CommitTool::new())),
        (ToolTier::Specialty, Arc::new(ReviewTool::new())),
        (ToolTier::Specialty, Arc::new(AskTool::new())),
        (ToolTier::Specialty, Arc::new(ImageGenTool::new())),
        (ToolTier::Specialty, Arc::new(todo_tool)),
        // Matrix
        (ToolTier::Matrix, Arc::new(MatrixSendTool::new())),
        // ... remaining matrix tools
    ]
}
```

## Activation Rules by Mode

| Mode | Active tiers | Rationale |
|------|-------------|-----------|
| `clankers -p "prompt"` (headless) | Core | Minimal token cost for one-shot tasks |
| Interactive TUI | Core + Specialty | Full editing experience without orchestration overhead |
| Interactive + `/worker` or `/share` | Core + Specialty + Orchestration | User requested swarm features |
| Daemon | All | Needs matrix, orchestration, everything |
| `--tools all` | All | Explicit override |
| `--tools core` | Core | Explicit minimal mode |
| Agent definition with `tiers` | As specified | Per-agent control |

## CLI Flag

```
--tools <MODE>    Tool mode: all, core, none (default: auto)
```

`auto` applies the activation rules above. `all` forces all tiers. `core`
forces core only. `none` disables all tools (existing behavior preserved).

## Agent Definition Integration

Agent definitions in `agents.yaml` can specify tool tiers:

```yaml
agents:
  scout:
    model: claude-sonnet-4-20250514
    tiers: [core]              # read-only recon agent
  worker:
    model: claude-sonnet-4-20250514
    tiers: [core, specialty]   # full implementation agent
  orchestrator:
    model: claude-opus-4-6
    tiers: [core, orchestration]  # delegates to workers
```

When `tiers` is absent, the mode default applies.

## Mid-Session Escalation

Not required for v1. The tier is set at session start and doesn't change
unless the user explicitly activates a tier via `/tools tier orchestration`
or similar. Mid-session escalation based on response text scanning is a
future optimization.

## Migration Path

1. Add `ToolTier` enum and `ToolSet` struct to `src/modes/common.rs`.
2. Refactor `build_tools_with_env()` → `build_all_tools()` returning
   `Vec<(ToolTier, Arc<dyn Tool>)>`.
3. Add `ToolSet::new()` that accepts active tiers.
4. Update all callers of `build_tools_with_env()`:
   - `src/modes/interactive.rs` → `ToolSet::new(env, [Core, Specialty])`
   - `src/commands/daemon.rs` → `ToolSet::new(env, [Core, Orchestration, Specialty, Matrix])`
   - `src/commands/rpc.rs` → `ToolSet::new(env, [Core, Orchestration, Specialty, Matrix])`
   - Headless `-p` mode → `ToolSet::new(env, [Core])`
5. Update the agent loop to call `tool_set.active_tools()` instead of using
   the flat vec directly.
6. Update `/tools` slash command to show tier information.

## Callers to Update

```
src/modes/common.rs:43        — build_tools_with_env (refactor to build_all_tools)
src/modes/interactive.rs       — interactive mode tool setup
src/commands/daemon.rs:33      — daemon mode tool setup
src/commands/rpc.rs:684        — RPC server tool setup
```

## Invariants

- Core tier is always active. There is no mode where core tools are disabled
  (use `--tools none` for that, which is a separate code path).
- Tool collision detection (`builtin_names` for plugins) uses `all_tools()`,
  not `active_tools()`. A plugin tool that collides with a tier-2 tool still
  gets flagged even if tier 2 is inactive.
- The `switch_model` tool moves from tier 2 (specialty) to tier 0 (core) if
  multi-model is configured. Otherwise it stays in tier 2. This avoids a
  ~120-token tool definition when single-model.

## Tests

- `tool_set_core_only`: active_tools returns only 7 tools when [Core] active.
- `tool_set_all_tiers`: active_tools returns all 25 when all tiers active.
- `tool_set_activate_deactivate`: tier activation/deactivation works.
- `tool_set_collision_uses_all`: builtin_names derived from all_tools, not active.
- `headless_mode_uses_core`: `-p` mode creates ToolSet with Core only.
- `interactive_mode_uses_core_specialty`: TUI creates ToolSet with Core + Specialty.
- `daemon_mode_uses_all`: daemon mode creates ToolSet with all tiers.
