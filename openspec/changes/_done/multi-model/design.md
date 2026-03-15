# multi-model — Design

## Decisions

### Routing happens at turn granularity, not message-level

**Choice:** Model selection runs once at the start of each turn (when the user
sends a message) and optionally during the turn if the agent explicitly
requests a switch.

**Rationale:** Message-level routing (switching models mid-response) breaks
conversation context — the new model hasn't seen the agent's internal reasoning.
Turn-level routing is the natural boundary: the user asks a question, we pick
the right model, and that model handles the entire turn. Agent-initiated
switching mid-turn is allowed (agent calls `switch_model` tool) but happens
explicitly, not automatically.

**Alternatives considered:**
- **Token-level**: Switch models per token streamed. Impossible — streaming is
  opaque and stateful.
- **Tool-call-level**: Switch models around each tool call. Breaks context and
  makes the conversation incoherent. The model loses track of what it was doing.
- **Message-level**: Switch after each assistant/user message pair. Technically
  feasible but context loss is high. The new model doesn't know what the prior
  model was thinking.

### Agent can request model switches via a tool, not implicit detection

**Choice:** Add a `switch_model` tool the agent can call explicitly. The tool
takes a `role` parameter (e.g., "slow", "smol") and a `reason` string. The
router validates the request, switches the model, and the agent continues in
the same turn.

**Rationale:** Explicit is better than implicit. The agent knows when it's stuck
or when a task is trivial. Let it ask for help. Implicit detection (heuristics
like "agent used the word 'complex'") is brittle and leads to false positives.

**Alternatives considered:**
- **Implicit keyword detection**: If the agent says "this requires deep
  thinking", auto-switch to opus. Fragile — the agent might say that
  sarcastically or in a quote.
- **Retry on error**: If the agent fails a tool call, auto-switch to a smarter
  model and retry. Sounds good but error != needs smarter model. Might just
  need a different approach, not more intelligence.

### Cost tracking uses a token-based model, not API billing

**Choice:** Track tokens (input + output) per model, multiply by hardcoded
per-token costs, sum to get session spend.

**Rationale:** We control the token counts (they're in the API response). We
don't control when Anthropic bills us or what their batch discounts are. Token
math is deterministic and transparent. Users can see "you've used 50k tokens on
opus at $15/MTok = $0.75".

**Alternatives considered:**
- **Poll Anthropic billing API**: No real-time API. The usage dashboard updates
  hourly. Can't use this for in-session budget enforcement.
- **Estimate via prompt length**: Inaccurate. Tokenization isn't word count.
- **Flat rate per turn**: Doesn't account for long outputs or tool use.

### Budget thresholds trigger warnings, not hard stops

**Choice:** When a cost threshold is hit (e.g., $5 spent), emit a warning event
and optionally downgrade to cheaper models. But **never halt mid-turn** or
refuse to call the model.

**Rationale:** Hard stops are surprising and break the user's flow. "Sorry, I've
hit my budget and can't respond" is a bad UX. Better to warn ("you've spent $5,
switching to haiku for the rest of this session") and let the user decide
whether to continue.

**Alternatives considered:**
- **Hard stop**: Refuse to call the API when budget is exceeded. User has to
  restart with a higher budget. Annoying and breaks in-progress work.
- **Ask for permission**: Pause and prompt "Continue at $X/turn? [y/n]". Breaks
  the conversation flow and interrupts the agent mid-thought.

### Orchestration uses sequential model handoffs, not parallel voting

**Choice:** For multi-model patterns (propose/validate), run models sequentially:
Model A generates a draft, Model B reviews and refines. The final output is
Model B's result.

**Rationale:** Sequential is simple, deterministic, and fits the turn model.
Parallel voting (multiple models independently solve the problem, pick the best)
requires running N models per turn — expensive and complicated. Sequential
leverages each model's strengths: fast model for boilerplate, slow model for
critical logic.

**Alternatives considered:**
- **Parallel voting**: Run haiku, sonnet, opus in parallel, pick the best via
  another model judging. 3x the cost, unclear which answer is "best" without
  ground truth.
- **Ensemble blending**: Merge outputs from multiple models. Works for numeric
  predictions, not code generation or prose.

### Complexity heuristics are explicit and configurable, not ML-based

**Choice:** Complexity score is a weighted sum of explicit signals:
- Token count (longer prompts → more complex)
- Tool call types (bash, edit → simple; delegate, subagent → complex)
- Keyword hints ("refactor", "architecture" → complex; "list", "grep" → simple)
- User role hint ("use opus", "fast please")

Users can tune weights in `RoutingPolicyConfig`.

**Rationale:** Transparent and debuggable. Users can see why a model was chosen.
ML-based complexity prediction (train a classifier on past turns) is overkill
and requires training data we don't have.

**Alternatives considered:**
- **ML classifier**: Train on historical turns to predict complexity. Needs
  labeled data, black box, hard to debug.
- **Ask the agent**: Have the agent score its own task complexity. Circular —
  need a model to pick a model.

### Router leverages existing `clankers-router` fallback chains

**Choice:** The RoutingPolicy selects a ModelRole (e.g., Slow → opus-4). The
router's existing fallback chain kicks in if opus-4 is unavailable (circuit
breaker tripped, API down). It tries opus-4-backup, then sonnet, then haiku.

**Rationale:** Reuse what's already there. The router already handles provider
failures. This change adds **intentional** model selection; the router adds
**resilience** when the selected model is unavailable.

**Alternatives considered:**
- **Bypass fallback for policy-selected models**: If the policy says "use opus"
  and opus is down, fail hard. Bad UX — unavailability shouldn't block work.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                        Agent Turn Loop                        │
│                                                              │
│  User Input                                                  │
│      │                                                       │
│      ▼                                                       │
│  ┌─────────────────────────────────────────────────────┐     │
│  │  RoutingPolicy::select_model()                      │     │
│  │  - Compute complexity score from:                   │     │
│  │    • Token count                                    │     │
│  │    • Tool call history (last N turns)               │     │
│  │    • Keywords in prompt                             │     │
│  │    • Explicit user hint                             │     │
│  │  - Check cost budget, downgrade if needed           │     │
│  │  - Return ModelRole                                 │     │
│  └────────────────┬────────────────────────────────────┘     │
│                   │                                          │
│                   ▼                                          │
│  ┌─────────────────────────────────────────────────────┐     │
│  │  Swap Agent's provider to selected model            │     │
│  │  - Resolve ModelRole → model_id via ModelRolesConfig │     │
│  │  - Create new provider from router                  │     │
│  │  - Emit AgentEvent::ModelChange { from, to, reason } │     │
│  └────────────────┬────────────────────────────────────┘     │
│                   │                                          │
│                   ▼                                          │
│  ┌─────────────────────────────────────────────────────┐     │
│  │  Agent executes turn with selected model            │     │
│  │  - Streaming response                                │     │
│  │  - Tool calls (bash, edit, etc.)                     │     │
│  │  - Agent may call switch_model tool                  │     │
│  └────────────────┬────────────────────────────────────┘     │
│                   │                                          │
│                   ▼                                          │
│  ┌─────────────────────────────────────────────────────┐     │
│  │  CostTracker::record_usage()                        │     │
│  │  - Log input/output tokens                          │     │
│  │  - Multiply by model pricing                        │     │
│  │  - Update session total                             │     │
│  │  - Check budget thresholds                          │     │
│  └─────────────────────────────────────────────────────┘     │
│                                                              │
│  Turn Complete                                               │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

### Turn Start — Automatic Model Selection

1. User sends message "Refactor the parser to use a combinator library"
2. `RoutingPolicy::select_model(&prompt, &history, &cost_tracker)` runs:
   - Token count: 80 (moderate)
   - Keywords: "refactor" (+10 complexity), "parser" (+5)
   - Tool history: Last turn used `edit` (simple)
   - Cost: $2.50 spent so far (under threshold)
   - **Score: 35 → selects ModelRole::Slow (opus)**
3. Agent's provider swapped to opus-4
4. `AgentEvent::ModelChange { from: "sonnet", to: "opus", reason: "complexity_score_35" }` emitted
5. Turn proceeds with opus

### Mid-Turn — Agent-Requested Switch

1. Agent realizes task is simpler than expected
2. Agent calls `switch_model(role="smol", reason="task is just listing files")`
3. Tool validates request (smol is cheaper than current model opus, allowed)
4. Provider swapped to haiku
5. `AgentEvent::ModelChange { from: "opus", to: "haiku", reason: "agent_request" }` emitted
6. Agent continues with haiku

### Budget Threshold — Cost-Aware Downgrade

1. Turn starts, cost tracker shows $4.80 spent (threshold: $5.00)
2. `RoutingPolicy::select_model()` sees budget near limit
3. Overrides complexity-based selection, picks ModelRole::Smol (haiku)
4. `AgentEvent::ModelChange { from: "sonnet", to: "haiku", reason: "budget_threshold" }` emitted
5. User sees warning in TUI: "Budget threshold reached ($5), using faster models"

### Orchestration — Propose/Validate Pattern

1. User: "Write a complex parser with error recovery"
2. Policy selects **orchestration mode** (detected via "complex" + "write")
3. **Phase 1 — Draft**: Haiku generates boilerplate parser structure
4. **Phase 2 — Validate**: Opus reviews, adds error recovery logic, refines
5. Final output is Opus's refined version
6. Session logs show two `ModelChange` events (default→haiku, haiku→opus) with reason "orchestration_phase"
