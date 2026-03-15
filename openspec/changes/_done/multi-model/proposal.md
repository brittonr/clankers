# multi-model — Orchestrate Different Models Within a Single Session

## Intent

Clankers already supports multiple models via the `ModelRole` enum (Default,
Smol, Slow, Plan, Commit, Review) and the `/role` command lets you switch
between them manually. But the switching is **always manual** or **per-
subagent**. There's no automatic routing based on task complexity, no cost-
aware degradation when budget thresholds hit, no agent-initiated model
switching when it realizes it needs deeper thinking, and no multi-model
orchestration within a single turn (like using a cheap model to propose and
an expensive model to validate).

This change adds **dynamic model routing** so that:
- The router automatically selects the right model for the current task based on complexity signals
- Cost tracking prevents runaway spend by downgrading to cheaper models when budgets are hit
- The agent can request a different model mid-conversation when it realizes the task is harder than expected
- Multiple models can collaborate within a single turn (propose/validate, plan/execute, draft/review patterns)

## Scope

### In Scope

- **Routing policy engine** — Rules for automatic model selection based on:
  - Task complexity heuristics (token count, tool call patterns, user keywords)
  - Cost budgets and thresholds
  - Agent-initiated role requests (agent asks for a smarter/faster model)
  - Explicit user hints in prompts ("use opus for this", "fast response please")
- **Cost tracking** — Per-model usage tracking, cumulative spend, budget policies
- **Model switching events** — Enhanced `AgentEvent::ModelChange` with reason (automatic, budget, agent-requested, user-commanded)
- **Orchestration patterns** — Multi-model workflows within a turn:
  - **Propose/validate**: Haiku drafts, Sonnet reviews and corrects
  - **Plan/execute**: Opus plans architecture, Sonnet implements
  - **Draft/review**: Haiku generates boilerplate, Opus refines critical sections
- **Router integration** — Leverage existing `clankers-router` fallback chains and circuit breaker
- **Agent tool** — New `switch_model` tool the agent can call to request a different model
- **TUI indicator** — Show active model, cost-to-date, budget status in status bar
- **Session persistence** — Log all model switches in session JSONL with rationale

### Out of Scope

- Multi-model **parallel** execution (running two models simultaneously on different tasks)
  - Subagents/delegates already support this — they spawn separate Agent instances
- Fine-tuned or custom model training
- Modifying the Provider trait or streaming protocol
- Cross-provider orchestration (mixing Anthropic + OpenAI in a single turn)
  - The router already handles provider fallback; this focuses on model selection within a provider
- Real-time model benchmarking or A/B testing
- User-defined routing DSL (future: allow users to write custom routing rules in config)

## Approach

A **RoutingPolicy** struct holds the rules for automatic model selection. At
key decision points (turn start, tool planning, agent request), the policy
evaluates **complexity signals** (token count, tool types, keywords, agent
hints) and selects the appropriate ModelRole. The Agent's provider is swapped
mid-session to the new model, and an `AgentEvent::ModelChange` is emitted
with the reason.

**Cost tracking** runs in a background task that accumulates usage per model
(input/output tokens × per-model pricing). When a threshold is hit (e.g., $5
spent this session), the policy downgrades to cheaper models or warns the user.

**Orchestration** is implemented as phased turn execution: the agent yields
intermediate results (e.g., a draft), the router spawns a second model to
validate/refine, and the final result is returned. The entire interaction is
logged as a single turn with multiple model segments.

Existing infrastructure (ModelRole, clankers-router, AgentEvent, session JSONL)
is reused. No changes to the Provider trait or tool execution model — this is
pure routing logic on top of what exists.
