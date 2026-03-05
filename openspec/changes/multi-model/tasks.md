# multi-model — Tasks

## Phase 1: Core routing policy (no orchestration, no cost tracking)

- [ ] Create `src/routing/` module structure
- [ ] Implement `ComplexitySignals` struct with token count, tool history, keywords
- [ ] Implement `RoutingPolicy` struct with complexity scoring logic
- [ ] Implement `RoutingPolicyConfig` with thresholds and weights
- [ ] Add default keyword hints map (complexity increasers/reducers)
- [ ] Implement `RoutingPolicy::compute_complexity()` with weighted scoring
- [ ] Implement `RoutingPolicy::select_model()` returning `ModelSelectionResult`
- [ ] Add `SelectionReason` enum for tracking why a model was chosen
- [ ] Integrate `RoutingPolicy` into agent turn loop (call before turn execution)
- [ ] Wire model switching when selected role differs from current
- [ ] Add `reason` field to existing `AgentEvent::ModelChange`
- [ ] Unit tests: complexity scoring, threshold mapping, keyword extraction
- [ ] Integration test: verify model switches based on prompt keywords

## Phase 2: Cost tracking and budget enforcement

- [ ] Implement `ModelPricing` struct with input/output costs per MTok
- [ ] Create default pricing table for Anthropic models (opus, sonnet, haiku)
- [ ] Implement pricing loader from `~/.clankers/pricing.json` (optional override)
- [ ] Implement `CostTracker` struct with per-model usage tracking
- [ ] Implement `CostTracker::record_usage()` called after each API response
- [ ] Implement token-to-cost conversion (tokens / 1M * price_per_mtok)
- [ ] Add `CostTrackerConfig` with soft/hard budget limits and warning intervals
- [ ] Implement threshold checking (soft limit warning, hard limit enforcement)
- [ ] Add `AgentEvent::CostUpdate`, `BudgetWarning`, `BudgetExceeded`, `CostMilestone`
- [ ] Implement `CostTracker::summary()` returning aggregate stats
- [ ] Implement `CostTracker::budget_status()` for TUI display
- [ ] Integrate cost tracking into agent: record usage after each turn
- [ ] Wire budget status into `RoutingPolicy::select_model()` (downgrade if over limit)
- [ ] Unit tests: cost calculation, threshold detection, budget status
- [ ] Integration test: verify downgrade when budget threshold hit

## Phase 3: Agent-initiated model switching

- [ ] Create `switch_model` tool in `src/tools/switch_model.rs`
- [ ] Implement tool parameters: `role` (ModelRole string), `reason` (justification)
- [ ] Validate switch request (disallow upgrade from cheap to expensive if over budget)
- [ ] Emit `AgentEvent::ModelChange` with `reason: "agent_request"`
- [ ] Update agent's current model mid-turn (swap provider)
- [ ] Add `switch_model` to system prompt tool descriptions
- [ ] Add agent examples: when to switch (task simpler/harder than expected)
- [ ] Unit tests: validation logic, rejected upgrades over budget
- [ ] Integration test: agent calls tool, model switches, turn continues

## Phase 4: TUI cost display

- [ ] Add cost summary to status bar: `[model] tokens | $cost | Budget: $X / $Y`
- [ ] Color-code budget status (green: ok, yellow: warning, red: exceeded)
- [ ] Show current model name with role indicator (e.g., `[sonnet·default]`)
- [ ] Handle `AgentEvent::CostUpdate` to refresh cost display
- [ ] Handle `AgentEvent::BudgetWarning` to flash yellow indicator
- [ ] Add budget bar component (optional): visual progress toward limit
- [ ] Add cost breakdown panel (optional): per-model usage table
- [ ] Keybinding to toggle cost detail view (e.g., `C`)

## Phase 5: Cost inspection tool (agent self-awareness)

- [ ] Create `cost` tool in `src/tools/cost.rs`
- [ ] Implement `list` action: show per-model breakdown
- [ ] Implement `summary` action: one-line total and budget status
- [ ] Implement `budget` action: remaining budget, projected turns left
- [ ] Register `cost` tool in `src/tools/mod.rs`
- [ ] Add tool to system prompt descriptions
- [ ] Unit tests: each action with mock cost tracker state
- [ ] Integration test: agent calls tool, receives cost info

## Phase 6: Orchestration (experimental, disabled by default)

- [ ] Implement `OrchestrationPlan` struct with pattern and phases
- [ ] Implement `OrchestrationPattern` enum (ProposeValidate, PlanExecute, DraftReview)
- [ ] Implement `OrchestrationPhase` struct with role, system suffix, previous output
- [ ] Add orchestration detection to `RoutingPolicy::select_model()`
- [ ] Implement `RoutingPolicy::plan_orchestration()` for ProposeValidate pattern
- [ ] Define system prompt suffixes for each phase (PROPOSE_SUFFIX, VALIDATE_SUFFIX, etc.)
- [ ] Implement `Agent::execute_orchestrated_turn()` with phase loop
- [ ] Implement `Agent::build_phase_prompt()` to include previous phase output
- [ ] Log each orchestration phase in session JSONL
- [ ] Add `entry_type: orchestration_start/phase/complete` to session log
- [ ] Wire orchestration into agent turn loop (if plan is present)
- [ ] Add `enable_orchestration` flag to `RoutingPolicyConfig` (default: false)
- [ ] Unit tests: pattern detection, phase prompt building
- [ ] Integration test: full ProposeValidate workflow, verify cost savings

## Phase 7: User hints and explicit role requests

- [ ] Implement `ModelRoleHint` enum (Explicit, Fast, Thorough)
- [ ] Implement `parse_user_hint()` to extract hints from prompt
- [ ] Detect explicit role requests: "use opus", "use haiku", "fast please"
- [ ] Detect complexity hints: "think deeply", "quick answer"
- [ ] Give user hints highest priority in model selection
- [ ] Add hint examples to documentation
- [ ] Unit tests: hint parsing, priority override
- [ ] Integration test: user says "use opus", verify opus is selected

## Phase 8: Configuration and persistence

- [ ] Add `routingPolicy` section to `settings.json` schema
- [ ] Serialize/deserialize `RoutingPolicyConfig` from settings
- [ ] Add `costTracking` section to `settings.json` schema
- [ ] Serialize/deserialize `CostTrackerConfig` from settings
- [ ] Persist budget limits across sessions (optional: separate budget file)
- [ ] Add CLI flags: `--budget-limit`, `--enable-orchestration`, `--routing-policy`
- [ ] Document configuration options in README
- [ ] Example configs for different use cases (cost-conscious, quality-first, balanced)

## Phase 9: Documentation and examples

- [ ] Document multi-model feature in main README
- [ ] Create `docs/multi-model.md` with detailed guide
- [ ] Add examples of automatic routing scenarios
- [ ] Add examples of agent-initiated switching
- [ ] Add cost optimization tips (when to use haiku vs opus)
- [ ] Document orchestration patterns with cost comparisons
- [ ] Add troubleshooting section (unexpected model switches, budget issues)
- [ ] Update CHANGELOG.md

## Phase 10: Testing and validation

- [ ] End-to-end test: complex task auto-selects opus
- [ ] End-to-end test: simple task auto-selects haiku
- [ ] End-to-end test: budget threshold triggers downgrade
- [ ] End-to-end test: agent calls switch_model successfully
- [ ] End-to-end test: orchestration runs two phases correctly
- [ ] Performance test: routing overhead is <10ms per turn
- [ ] Cost accuracy test: compare tracked cost vs Anthropic API dashboard
- [ ] Load test: many rapid model switches don't leak memory or connections
