# Design: Token Efficiency

## 1. Tiered Tool Registration

### Current State

`build_tools_with_env()` in `src/modes/common.rs` returns a flat `Vec<Arc<dyn Tool>>`
with all 25 tools. Every API call sends all 25 tool schemas regardless of
whether the user is writing a haiku or orchestrating a multi-agent workflow.

### Tiers

```
Tier 0 (core, always registered):     ~7 tools, ~2,500 tokens
  read, write, edit, bash, grep, find, ls

Tier 1 (orchestration, on demand):    ~4 tools, ~1,800 tokens
  subagent, delegate_task, signal_loop_success, procmon

Tier 2 (specialty, on demand):        ~8 tools, ~1,800 tokens
  nix, web, commit, review, ask, image_gen, todo, switch_model

Tier 3 (matrix, daemon-only):         ~6 tools, ~1,200 tokens
  matrix_send, matrix_read, matrix_rooms, matrix_peers, matrix_join, matrix_rpc
```

### Activation Rules

| Tier | Activated when |
|------|---------------|
| 0 | Always |
| 1 | `--swarm` flag, agent definition requests it, or model mentions "delegate"/"subagent" in response |
| 2 | Always in interactive mode; filtered in headless/`-p` mode based on prompt content heuristics |
| 3 | Daemon mode only, or `--matrix` flag |

### Architecture

```rust
// src/modes/common.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolTier {
    Core,
    Orchestration,
    Specialty,
    Matrix,
}

pub struct ToolSet {
    tools: Vec<(ToolTier, Arc<dyn Tool>)>,
    active_tiers: HashSet<ToolTier>,
}

impl ToolSet {
    /// Build all tools but only activate specified tiers.
    pub fn new(env: &ToolEnv, tiers: &[ToolTier]) -> Self { ... }

    /// Tools currently active (sent to the API).
    pub fn active_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.iter()
            .filter(|(tier, _)| self.active_tiers.contains(tier))
            .map(|(_, tool)| tool.clone())
            .collect()
    }

    /// Activate a tier mid-session (e.g., when the model requests delegation).
    pub fn activate_tier(&mut self, tier: ToolTier) {
        self.active_tiers.insert(tier);
    }

    /// Deactivate a tier.
    pub fn deactivate_tier(&mut self, tier: ToolTier) {
        self.active_tiers.remove(&tier);
    }
}
```

### Mid-Session Tier Escalation

When the model's response text contains tool-request indicators (e.g., it says
"I'll delegate this" but has no delegate tool), the loop detects this and
activates the relevant tier for the next turn. This avoids pre-loading tools
"just in case" while still making them available when needed.

Detection is a simple keyword scan on the assistant's text output — not a
classifier, not an LLM call. False positives just load a few extra tools
for one turn; false negatives mean the model retries without the tool
(same as today when a tool isn't available).

```rust
fn detect_tier_escalation(response_text: &str) -> Vec<ToolTier> {
    let mut tiers = Vec::new();
    let lower = response_text.to_lowercase();
    if lower.contains("delegate") || lower.contains("subagent") || lower.contains("worker") {
        tiers.push(ToolTier::Orchestration);
    }
    if lower.contains("matrix") || lower.contains("send message") {
        tiers.push(ToolTier::Matrix);
    }
    tiers
}
```

### Backward Compatibility

- Interactive mode starts with tiers 0 + 2 (core + specialty). Same tools as
  today minus orchestration and matrix. Orchestration activates on first
  `/worker` or `/share` command, or when agent defs specify it.
- Daemon mode starts with tiers 0 + 1 + 2 + 3 (all tiers). No change.
- `--tools all` flag forces all tiers active.
- Agent definitions can specify `tiers: ["core", "orchestration"]` to control
  which tiers are active.

## 2. System Prompt Trimming

### Current State

`default_system_prompt()` in `crates/clankers-agent/src/system_prompt.rs`
returns a static string with ~799 tokens covering:

| Section | Tokens (approx) | When relevant |
|---------|----------------:|--------------|
| Base (role, guidelines) | ~200 | Always |
| Nix package management | ~180 | When commands not found |
| Model switching | ~120 | When using multi-model |
| HEARTBEAT.md | ~80 | Daemon mode only |
| Process monitoring | ~80 | When long-running processes |
| **Total** | **~660** | |

(Plus ~140 tokens of formatting/whitespace overhead.)

### Target State

The base prompt stays at ~200 tokens. Conditional sections are appended only
when their feature is active:

```rust
pub fn build_system_prompt(features: &ActiveFeatures) -> String {
    let mut prompt = BASE_PROMPT.to_string();

    if features.nix_available {
        prompt.push_str(NIX_SECTION);
    }
    if features.multi_model {
        prompt.push_str(MODEL_SWITCHING_SECTION);
    }
    if features.daemon_mode {
        prompt.push_str(HEARTBEAT_SECTION);
    }
    if features.process_monitor {
        prompt.push_str(PROCMON_SECTION);
    }

    prompt
}

pub struct ActiveFeatures {
    pub nix_available: bool,    // check `which nix` at startup
    pub multi_model: bool,      // true if model roles configured
    pub daemon_mode: bool,      // true in daemon/rpc mode
    pub process_monitor: bool,  // true if procmon tool is in active tier
}
```

### Detection

- `nix_available`: `which nix` at startup, cached for session.
- `multi_model`: true if settings contain model role overrides or multiple
  providers are configured.
- `daemon_mode`: set by the daemon/rpc command path.
- `process_monitor`: true if ToolTier::Orchestration is active (ties into
  tiered tools).

## 3. Tool Output Truncation

### Current State

Tool results are returned verbatim. A `bash: grep -rl "pattern" .` in a large
repo can return megabytes. The tool output becomes part of the conversation
context and is re-sent on every subsequent turn.

### Target State

All tool results pass through a truncation layer before being added to the
conversation:

```rust
pub struct TruncationConfig {
    pub max_bytes: usize,      // default: 50 * 1024 (50KB)
    pub max_lines: usize,      // default: 2000
}

impl Default for TruncationConfig {
    fn default() -> Self {
        Self {
            max_bytes: 50 * 1024,
            max_lines: 2000,
        }
    }
}
```

When output exceeds either limit:

1. Save full output to a temp file (e.g., `/tmp/clankers-tool-output-XXXX.txt`).
2. Truncate to the limit (whichever is hit first).
3. Append a footer:

```
[Output truncated: 4,231 lines / 182KB. Full output saved to /tmp/clankers-tool-output-a1b2c3.txt]
[Use `read /tmp/clankers-tool-output-a1b2c3.txt` with offset/limit to see the rest]
```

### Where to Apply

The truncation layer sits in the agent loop, between tool execution and
message assembly. Tools return raw output; the loop truncates before
adding to conversation history.

```rust
// crates/clankers-loop/src/lib.rs (or wherever tool results are processed)

fn process_tool_result(result: ToolResult, config: &TruncationConfig) -> ToolResult {
    let content = result.content();
    let lines: Vec<&str> = content.lines().collect();
    let bytes = content.len();

    if lines.len() <= config.max_lines && bytes <= config.max_bytes {
        return result; // No truncation needed
    }

    // Determine truncation point
    let mut kept_bytes = 0;
    let mut kept_lines = 0;
    for line in &lines {
        if kept_lines >= config.max_lines || kept_bytes + line.len() > config.max_bytes {
            break;
        }
        kept_bytes += line.len() + 1; // +1 for newline
        kept_lines += 1;
    }

    // Save full output
    let tmp_path = save_to_temp(content);

    // Build truncated result
    let truncated = format!(
        "{}\n\n[Output truncated: {} lines / {}. Full output saved to {}]\n[Use `read {}` with offset/limit to see the rest]",
        lines[..kept_lines].join("\n"),
        lines.len(),
        format_bytes(bytes),
        tmp_path.display(),
        tmp_path.display(),
    );

    result.with_content(truncated)
}
```

### Tool-Specific Overrides

Some tools already handle their own truncation (e.g., `read` with offset/limit).
The truncation layer is a safety net, not a replacement for tool-level smarts.
Tools can opt out by marking their result as `pre_truncated: true`.

## Impact Estimate

Based on the 10-prompt benchmark:

| Change | Token savings | Per-prompt avg |
|--------|-------------:|---------------:|
| Tiered tools (core only in headless) | ~70K | ~7,000 |
| System prompt trim | ~4K | ~400 |
| Output truncation | ~0 (clankers) | ~0 (already efficient) |
| **Total** | **~74K** | **~7,400** |

The output truncation saves nothing in the current benchmark because clankers'
gitignore-aware grep already prevents blowups. It's insurance against future
catastrophic cases. For pi-like agents without gitignore awareness, it would
have saved ~126K tokens on prompt #3 alone.

After these changes, the benchmark should show clankers at roughly 45K total
tokens (down from 116K), a 2.6× improvement.
