# Multi-Model Conversations

Clankers automatically routes tasks to the right model based on complexity, tracks costs per-model, and supports optional multi-phase orchestration where different models collaborate on a single turn.

**Key benefits:**
- Save money on simple tasks by using cheaper models
- Get better results on complex tasks by upgrading to more capable models  
- Budget enforcement prevents runaway costs
- Agent can switch models mid-conversation when complexity changes
- Transparent cost tracking with per-model breakdown

## Quick Start

Enable routing in your settings file (`~/.clankers/settings.json` or `.clankers/settings.json`):

```json
{
  "routing": {
    "enabled": true,
    "low_threshold": 20.0,
    "high_threshold": 50.0,
    "budget_soft_limit": 5.0,
    "budget_hard_limit": 10.0
  },
  "modelRoles": {
    "default": { 
      "name": "default",
      "description": "General-purpose tasks",
      "model": "claude-sonnet-4-5"
    },
    "smol": {
      "name": "smol", 
      "description": "Simple/fast tasks",
      "model": "claude-haiku-4"
    },
    "slow": {
      "name": "slow",
      "description": "Complex reasoning",
      "model": "claude-opus-4"
    }
  }
}
```

That's it. The agent will now:
- Use Haiku for simple tasks (grep, list files, read)
- Use Sonnet for balanced general work
- Use Opus for complex analysis and refactoring
- Warn you at $5 spent
- Force cheaper models after $10

## How Routing Works

### Complexity Scoring

Every user message is scored based on:

1. **Token count** — Longer prompts score higher  
   Formula: `(token_count / 50.0) * token_weight`  
   Default weight: 1.0

2. **Tool complexity** — Recent tool calls add weight  
   - Simple tools (read, ls, grep, find): +1.0 each
   - Medium tools (bash, edit, write): +3.0 each  
   - Complex tools (subagent, delegate): +10.0 each

3. **Keyword hints** — Specific words adjust the score  
   Positive (increase complexity):
   - `refactor` +10.0
   - `architecture` +15.0
   - `security` +12.0
   - `design` +10.0
   - `complex` +10.0
   - `optimize` +8.0
   - `analyze` +8.0
   - `debug` +8.0

   Negative (decrease complexity):
   - `grep` -8.0
   - `find` -8.0
   - `quick` -10.0
   - `simple` -8.0
   - `list` -5.0
   - `show` -5.0
   - `read` -5.0

**Example scoring:**
```
Prompt: "list all files in src/"
Token count: 50 → 1.0
Keywords: "list" → -5.0
Total: -4.0 → routes to "smol" (haiku)

Prompt: "refactor the auth system for security"
Token count: 500 → 10.0  
Keywords: "refactor" +10.0, "security" +12.0
Total: 32.0 → routes to "default" (sonnet)

Prompt: "design a complex distributed cache architecture"
Token count: 1000 → 20.0
Keywords: "design" +10.0, "complex" +10.0, "architecture" +15.0
Total: 55.0 → routes to "slow" (opus)
```

### Model Selection

Based on the final score:

- **Score < 20.0** → `smol` role (Haiku, fast/cheap)
- **20.0 ≤ Score ≤ 50.0** → `default` role (Sonnet, balanced)
- **Score > 50.0** → `slow` role (Opus, powerful)

Thresholds are configurable via `low_threshold` and `high_threshold` in the routing config.

### User Hints Override

User can explicitly request a model tier:

- **"quick answer"** / **"quickly"** / **"fast response"** → forces `smol`
- **"think deeply"** / **"carefully"** / **"thorough"** / **"detailed analysis"** → forces `slow`
- **"use opus"** / **"switch to opus"** → forces `slow`
- **"use haiku"** / **"switch to haiku"** → forces `smol`
- **"use sonnet"** / **"switch to sonnet"** → forces `default`

User hints bypass complexity scoring but **do not** override hard budget limits.

### Budget Enforcement

Two budget thresholds control routing behavior:

#### Soft Limit
When exceeded, the routing policy **halves the complexity score**, biasing selection toward cheaper models. The agent can still use expensive models if the task demands it, but simple/medium tasks will downgrade.

Example with `budget_soft_limit: 5.0`:
```
Before soft limit (cost: $4.00):
  Score 55 → "slow" (opus)

After soft limit (cost: $6.00):
  Score 55 → adjusted to 27.5 → "default" (sonnet)
```

#### Hard Limit
When exceeded, the routing policy **forces the cheapest model** (`smol` role) regardless of complexity or user hints. This is an emergency brake to prevent runaway costs.

The agent's `switch_model` tool will also reject any upgrade requests when over the hard limit (but allows downgrades).

**Status bar indicator:**
- Green badge: under budget
- Yellow badge with `⚠`: over soft limit
- Red badge with `✖`: over hard limit

## Model Roles

Roles map abstract task categories to concrete model IDs. Six roles are built-in:

| Role      | Purpose                                  | Default Model    |
|-----------|------------------------------------------|------------------|
| `default` | General-purpose tasks                    | (not set)        |
| `smol`    | Simple/fast tasks (file ops, grep, etc.) | (not set)        |
| `slow`    | Complex reasoning and analysis           | (not set)        |
| `plan`    | Architecture and planning                | (not set)        |
| `commit`  | Commit message generation                | (not set)        |
| `review`  | Code review and analysis                 | (not set)        |

### Role Aliases

For convenience, several aliases map to builtin roles:

- `fast`, `small` → `smol`
- `large`, `thinking` → `slow`
- `planning`, `architect` → `plan`
- `git` → `commit`
- `code-review` → `review`

### Configuring Roles

Roles are configured in `modelRoles` section of settings:

```json
{
  "modelRoles": {
    "default": {
      "name": "default",
      "description": "General-purpose tasks",
      "model": "claude-sonnet-4-5"
    },
    "smol": {
      "name": "smol",
      "description": "Fast and cheap",
      "model": "claude-haiku-4"
    },
    "slow": {
      "name": "slow", 
      "description": "Deep thinking",
      "model": "claude-opus-4"
    }
  }
}
```

If a role's `model` field is omitted, it inherits from the `default` role, which in turn falls back to the global `model` setting.

### Custom Roles

Add your own roles for specialized tasks:

```json
{
  "modelRoles": {
    "debug": {
      "name": "debug",
      "description": "Debugging and tracing",
      "model": "claude-sonnet-4-5",
      "keywords": ["debug", "trace", "backtrace", "panic", "segfault"]
    },
    "docs": {
      "name": "docs",
      "description": "Documentation writing",
      "model": "claude-sonnet-4-5",
      "keywords": ["document", "readme", "api docs", "comment"]
    }
  }
}
```

**Keywords** are used for auto-inference. If the user prompt contains any keyword for a role, that role is selected. The first match wins (roles are checked in insertion order).

## Cost Tracking

Clankers tracks token usage and calculates cost for every model used in a session.

### Default Pricing

Built-in pricing for Claude models (cost per million tokens):

| Model                   | Input $/MTok | Output $/MTok |
|-------------------------|--------------|---------------|
| claude-opus-4           | 15.00        | 75.00         |
| claude-sonnet-4-5       | 3.00         | 15.00         |
| claude-sonnet-4         | 3.00         | 15.00         |
| claude-haiku-4          | 1.00         | 5.00          |
| claude-haiku-3-5        | 0.80         | 4.00          |

Dated model IDs (e.g., `claude-sonnet-4-5-20250514`) are matched by prefix.

### Custom Pricing

Override pricing in `~/.clankers/pricing.json`:

```json
{
  "claude-opus-4": {
    "input_per_mtok": 15.0,
    "output_per_mtok": 75.0,
    "display_name": "Claude Opus 4"
  },
  "gpt-4o": {
    "input_per_mtok": 5.0,
    "output_per_mtok": 15.0,
    "display_name": "GPT-4o"
  }
}
```

Models not in the pricing table are tracked at zero cost but still show token counts.

### Budget Configuration

Set limits in settings:

```json
{
  "routing": {
    "budget_soft_limit": 5.0,
    "budget_hard_limit": 10.0
  }
}
```

Or via CLI flag:

```bash
clankers --budget 10.0          # sets hard limit
clankers --max-cost 5.0         # sets hard limit (alias)
```

Budget events are logged to the conversation:

- **Soft limit crossed:** yellow warning badge in status bar
- **Hard limit crossed:** red exceeded badge, routing forced to cheapest model
- **Milestones:** optional warnings at regular intervals (e.g., every $1.00)

Configure milestone warnings:

```json
{
  "costTracking": {
    "soft_limit": 5.0,
    "hard_limit": 10.0,
    "warning_interval": 1.0
  }
}
```

### Viewing Costs

**Status bar** (always visible):
- Shows current session cost with color-coded budget indicator
- Format: `[$0.42 ($4.58 left)]` (green = OK, yellow = warning, red = exceeded)

**Slash commands:**
```
/usage                # detailed breakdown with per-model table
/status               # includes cost in session info
```

**Agent tools:**
The agent has a `cost` tool for self-awareness:

```
cost(action="summary")     # one-line total
cost(action="breakdown")   # per-model table
cost(action="budget")      # budget status and projection
```

Example agent use case:
```
User: "Run 50 test scenarios"
Agent: Let me check the budget first...
      [calls cost(action="budget")]
      Result: $8.50 spent, $1.50 until hard limit
Agent: I'm near the budget limit. I'll switch to Haiku for these tests.
      [calls switch_model(role="smol", reason="conserve budget")]
```

## Agent Tools

### switch_model

The agent can request a model switch mid-conversation:

```
switch_model(role="smol", reason="task simpler than expected")
switch_model(role="slow", reason="need deep reasoning for edge cases")
```

**Parameters:**
- `role` (required): target role name (`smol`, `default`, `slow`, or custom)
- `reason` (required): justification (logged for transparency)

**Budget enforcement:**
- Upgrades (e.g., haiku→opus) are **blocked** when over hard limit
- Downgrades (e.g., opus→haiku) are **always allowed**

The switch takes effect on the **next response** in the same turn loop.

**Example flow:**
```
1. User asks to "analyze this codebase"
2. Agent starts with Sonnet (default)
3. Agent calls read() on several files
4. Agent realizes the code is simple
5. Agent calls switch_model(role="smol", reason="straightforward refactor")
6. Agent continues with Haiku for the response
```

### cost

The agent can inspect session costs:

```
cost(action="summary")       # Total cost across all models
cost(action="breakdown")     # Per-model table with token counts
cost(action="budget")        # Budget status and remaining
```

**Returns:**
- `summary`: one-line string like "Session cost: $1.2345 across 2 model(s). Budget OK — $3.77 remaining."
- `breakdown`: formatted table with columns for model, input/output tokens, cost, and percentage
- `budget`: detailed budget status with projections

**Use cases:**
- Check before spawning expensive subagents
- Decide whether to switch to a cheaper model
- Warn the user when approaching limits

## Orchestration (Experimental)

Orchestration runs a single user turn in **multiple phases**, each using a different model optimized for that phase's goal. It's disabled by default and requires explicit opt-in.

**Enable in settings:**
```json
{
  "routing": {
    "enable_orchestration": true
  }
}
```

### Patterns

Three orchestration patterns are available:

#### ProposeValidate
Fast model drafts, slow model refines.

**Phases:**
1. **Propose** (smol/haiku): Generate a working draft quickly
2. **Validate** (slow/opus): Review, fix bugs, add error handling

**Use when:** Complex code generation where speed + quality both matter

**Example:**
```
User: "propose and validate a new parser for the config format"
Phase 1 (Haiku): [generates basic parser in ~5 seconds]
Phase 2 (Opus):  [reviews, adds error cases, improves handling]
```

#### PlanExecute
Slow model architects, medium model implements.

**Phases:**
1. **Plan** (slow/opus): Create detailed implementation plan
2. **Execute** (default/sonnet): Implement the plan step-by-step

**Use when:** Large refactors or greenfield features needing upfront design

**Example:**
```
User: "plan and implement a new auth system"
Phase 1 (Opus):   [writes architecture doc with steps]
Phase 2 (Sonnet): [implements following the plan]
```

#### DraftReview
Fast model writes boilerplate, medium model polishes.

**Phases:**
1. **Draft** (smol/haiku): Generate bulk content quickly
2. **Review** (default/sonnet): Improve clarity, fix errors, add examples

**Use when:** Documentation, test generation, or any content-heavy task

**Example:**
```
User: "draft and review API docs for the routing module"
Phase 1 (Haiku):  [writes comprehensive but rough docs]
Phase 2 (Sonnet): [polishes language, adds missing details]
```

### Triggering Orchestration

Orchestration is triggered in two ways:

1. **Explicit user hints** (always trigger):
   - "propose and validate X"
   - "draft and refine X"
   - "plan and implement X"
   - "plan and execute X"
   - "draft and review X"
   - "write and review X"

2. **Automatic heuristics** (only at high complexity):
   - Complexity score > 60.0
   - Prompt contains both generation keywords (`write`, `implement`, `create`, `build`) and architecture keywords (`refactor`, `architecture`, `design`, `complex`)
   - Auto-selects ProposeValidate pattern

### How It Works

1. The routing policy detects orchestration criteria
2. An `OrchestrationPlan` is created with ordered phases
3. Each phase:
   - Resolves the phase's role to a model ID
   - Appends a phase-specific system prompt suffix
   - Runs a full LLM call with the conversation history
   - Adds the phase's response to the conversation
4. The final phase's output is returned to the user

**Cost:** Each phase costs as if it were a separate turn. A two-phase orchestration uses ~2x the tokens of a single-turn response.

**Context:** All phases share the same conversation history. The second phase sees the first phase's output in the conversation.

### System Prompt Suffixes

Each phase gets a specialized system prompt addition:

**Propose phase:**
```
## Phase: Propose

Generate a working draft solution quickly. Prioritize:
- Covering the main logic flow
- Basic error handling
- Correctness of the core approach

A second model will review and refine your work, so don't over-engineer.
```

**Validate phase:**
```
## Phase: Validate

A draft solution was generated in the previous assistant message. Your goal:
- Review for correctness, safety, and edge cases
- Add comprehensive error handling
- Fix bugs or unsafe patterns
- Improve clarity and performance

Preserve what works. Output the final refined version.
```

Similar suffixes exist for Plan, Execute, Draft, and Review phases.

## Configuration Reference

### Full settings.json Example

```json
{
  "model": "claude-sonnet-4-5",
  "routing": {
    "enabled": true,
    "low_threshold": 20.0,
    "high_threshold": 50.0,
    "token_weight": 1.0,
    "tool_weight": 1.0,
    "keyword_hints": {
      "refactor": 10.0,
      "architecture": 15.0,
      "grep": -8.0,
      "quick": -10.0
    },
    "budget_soft_limit": 5.0,
    "budget_hard_limit": 10.0,
    "enable_orchestration": false
  },
  "costTracking": {
    "soft_limit": 5.0,
    "hard_limit": 10.0,
    "warning_interval": 1.0
  },
  "modelRoles": {
    "default": {
      "name": "default",
      "description": "General-purpose tasks",
      "model": "claude-sonnet-4-5"
    },
    "smol": {
      "name": "smol",
      "description": "Simple/fast tasks",
      "model": "claude-haiku-4",
      "keywords": ["grep", "find", "list", "read", "ls"]
    },
    "slow": {
      "name": "slow",
      "description": "Complex reasoning",
      "model": "claude-opus-4",
      "keywords": ["complex", "refactor", "think", "analyze"]
    },
    "plan": {
      "name": "plan",
      "description": "Architecture and planning",
      "model": "claude-opus-4",
      "keywords": ["plan", "architect", "design"]
    },
    "commit": {
      "name": "commit",
      "description": "Commit messages",
      "model": "claude-haiku-4",
      "keywords": ["commit", "changelog", "git"]
    },
    "review": {
      "name": "review",
      "description": "Code review",
      "model": "claude-sonnet-4-5",
      "keywords": ["review", "audit", "security"]
    }
  }
}
```

### Routing Policy Fields

| Field                  | Type               | Default | Description                                      |
|------------------------|--------------------|---------|--------------------------------------------------|
| `enabled`              | boolean            | true    | Enable/disable routing                           |
| `low_threshold`        | float              | 20.0    | Score below which `smol` is selected             |
| `high_threshold`       | float              | 50.0    | Score above which `slow` is selected             |
| `token_weight`         | float              | 1.0     | Weight for token count in scoring               |
| `tool_weight`          | float              | 1.0     | Weight for tool complexity in scoring            |
| `keyword_hints`        | map<string, float> | (built-in) | Keywords → complexity adjustment               |
| `budget_soft_limit`    | float (USD)        | null    | Warn and bias toward cheaper models              |
| `budget_hard_limit`    | float (USD)        | null    | Force cheapest model                             |
| `enable_orchestration` | boolean            | false   | Enable multi-phase orchestration                 |

### Cost Tracking Fields

| Field               | Type        | Default | Description                              |
|---------------------|-------------|---------|------------------------------------------|
| `soft_limit`        | float (USD) | null    | Soft budget threshold                    |
| `hard_limit`        | float (USD) | null    | Hard budget threshold                    |
| `warning_interval`  | float (USD) | null    | Emit warning every N dollars             |

### CLI Flags

| Flag                      | Description                                   |
|---------------------------|-----------------------------------------------|
| `--budget <amount>`       | Set hard budget limit (USD)                   |
| `--max-cost <amount>`     | Set hard budget limit (alias)                 |
| `--enable-routing`        | Enable routing (overrides settings)           |

### pricing.json Format

```json
{
  "<model-id>": {
    "input_per_mtok": 3.0,
    "output_per_mtok": 15.0,
    "display_name": "Human-readable name"
  }
}
```

Place in `~/.clankers/pricing.json`. Falls back to built-in pricing if not found or invalid.

## Troubleshooting

### Model not switching?

**Check role configuration:**
```
/status
```
Look for the `modelRoles` section. Ensure `smol`, `default`, and `slow` roles have `model` fields set.

**Verify routing is enabled:**
```json
{
  "routing": {
    "enabled": true
  }
}
```

**Check complexity score:**
Add logging to see scores:
```bash
RUST_LOG=clankers::routing=debug clankers
```

### Budget not being enforced?

**Soft vs hard limits:**
- Soft limit only **biases** selection, doesn't force
- Hard limit **forces** cheapest model

**Check settings:**
```json
{
  "routing": {
    "budget_hard_limit": 10.0  // must be set for hard enforcement
  }
}
```

**Verify cost tracking:**
```
/usage
```
If cost shows $0.00, pricing may not be configured for your models. Check `~/.clankers/pricing.json`.

### Orchestration not triggering?

**Must be explicitly enabled:**
```json
{
  "routing": {
    "enable_orchestration": true
  }
}
```

**Requires high complexity or explicit hint:**
- Automatic: score > 60.0 + generation + architecture keywords
- Manual: "propose and validate", "plan and implement", etc.

**Check logs:**
```bash
RUST_LOG=clankers::routing::orchestration=debug clankers
```

### Agent keeps using expensive model for simple tasks?

**Check user hints:**
If your prompt contains "think deeply", "carefully", "thorough", etc., the routing policy is forced to use `slow`.

**Lower the high_threshold:**
```json
{
  "routing": {
    "high_threshold": 40.0  // default is 50.0
  }
}
```

**Check keyword hints:**
Your prompt may contain complexity-increasing keywords. Review:
```json
{
  "routing": {
    "keyword_hints": {
      "your_keyword": 10.0  // this increases score
    }
  }
}
```

### Cost showing $0.00 but tokens are used?

**Model not in pricing table:**
- Add custom pricing in `~/.clankers/pricing.json`
- Or use a model with built-in pricing (see Default Pricing table)

**Prefix matching not working:**
Ensure your dated model ID starts with a pricing key:
- `claude-sonnet-4-5-20250514` matches `claude-sonnet-4-5` ✓
- `custom-sonnet-v2` matches nothing ✗

## Examples

### Simple file operations (routes to smol)

```
User: "list all rust files in src/"
Agent: [uses claude-haiku-4]
       [calls bash("find src/ -name '*.rs'")]
```

### Medium refactor (routes to default)

```
User: "refactor the auth module to use traits"
Agent: [uses claude-sonnet-4-5]
       [reads files, edits code, runs tests]
```

### Complex architecture (routes to slow)

```
User: "design a distributed cache with consistency guarantees"
Agent: [uses claude-opus-4]
       [thinks deeply, writes architecture doc]
```

### Mid-conversation downgrade

```
User: "analyze this codebase and list all the modules"
Agent: [starts with claude-sonnet-4-5]
       [reads a few files]
       "This is straightforward. Let me switch to a faster model."
       [calls switch_model(role="smol", reason="simple list operation")]
       [uses claude-haiku-4 for remainder]
```

### Budget-aware multi-step task

```
User: "refactor 20 files in src/"
Agent: [checks cost: $8.50 spent, $1.50 to hard limit]
       "I'm near the budget limit. I'll use Haiku for the bulk edits."
       [calls switch_model(role="smol", reason="conserve budget")]
       [completes task with claude-haiku-4]
```

### Orchestrated code generation

```
User: "propose and validate a TOML parser"
[Phase 1: Propose — claude-haiku-4]
Agent: [generates basic parser structure]
       [implements core parsing logic]
       [adds basic error handling]

[Phase 2: Validate — claude-opus-4]
Agent: [reviews phase 1 output]
       [adds comprehensive error handling]
       [fixes edge cases: nested tables, multiline strings]
       [improves performance with zero-copy parsing]
       [outputs final polished version]
```

### Custom role usage

```json
{
  "modelRoles": {
    "debug": {
      "name": "debug",
      "model": "claude-sonnet-4-5",
      "keywords": ["debug", "trace", "crash"]
    }
  }
}
```

```
User: "debug this segfault"
Agent: [auto-routed to "debug" role via keyword matching]
       [uses claude-sonnet-4-5]
```

## See Also

- [`commands.md`](commands.md) — `/model`, `/usage`, `/status` slash commands
- [`session-format.md`](session-format.md) — How conversations are persisted
- `crates/clanker-router/` — Multi-provider routing and caching
