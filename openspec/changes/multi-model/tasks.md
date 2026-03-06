# multi-model — Tasks

## Phase 1: Core routing policy (no orchestration, no cost tracking) ✅

- [x] Create `src/routing/` module structure (`mod.rs`, `config.rs`, `signals.rs`, `policy.rs`)
- [x] Implement `ComplexitySignals` struct with token count, tool history, keywords
- [x] Implement `RoutingPolicy` struct with complexity scoring logic
- [x] Implement `RoutingPolicyConfig` with thresholds and weights (serde-enabled)
- [x] Add default keyword hints map (complexity increasers/reducers)
- [x] Implement `RoutingPolicy::compute_complexity()` with weighted scoring
- [x] Implement `RoutingPolicy::select_model()` returning `ModelSelectionResult`
- [x] Add `SelectionReason` enum for tracking why a model was chosen
- [x] Integrate `RoutingPolicy` into agent turn loop (call before turn execution)
- [x] Wire model switching when selected role differs from current
- [x] Add `reason` field to existing `AgentEvent::ModelChange`
- [x] Unit tests: 18 tests covering scoring, thresholds, keywords, user hints, tool classification
- [x] `parse_user_hint()` detects "use opus/haiku/sonnet", "quick answer", "think deeply"
- [x] `classify_tool()` maps tools to Simple/Medium/Complex tiers
- [x] `recent_tool_summaries()` on Agent extracts recent tool calls for complexity signals

## Phase 2: Cost tracking and budget enforcement ✅

- [x] Implement `ModelPricing` struct with input/output costs per MTok
- [x] Create default pricing table for Anthropic models (opus 4, sonnet 4/4.5, haiku 3.5/4)
- [x] Implement pricing loader from `~/.clankers/pricing.json` (optional override, with prefix matching for dated model IDs)
- [x] Implement `CostTracker` struct with per-model usage tracking (thread-safe via `RwLock`)
- [x] Implement `CostTracker::record_usage()` called after each turn in `run_turn_loop`
- [x] Implement token-to-cost conversion (tokens / 1M * price_per_mtok)
- [x] Add `CostTrackerConfig` with soft/hard budget limits and warning intervals (serde-enabled)
- [x] Implement threshold checking — `BudgetEvent::Warning`, `Exceeded`, `Milestone` (fires once per crossing)
- [x] `CostTracker::summary()` returns `CostSummary` with per-model breakdown, percentages, budget status
- [x] `CostTracker::budget_status()` returns `BudgetStatus` enum (NoBudget/Ok/Warning/Exceeded)
- [x] `CostTracker::status_line()` returns formatted one-liner for status bar
- [x] Integrated into agent: `with_cost_tracker()` builder, passed to `run_turn_loop`
- [x] Wired budget into `RoutingPolicy::select_model()` via `budget_soft_limit`/`budget_hard_limit` on config
- [x] Soft budget halves complexity score (biases toward cheaper models)
- [x] Hard budget forces "smol" role regardless of complexity or user hints
- [x] `SelectionReason::BudgetThreshold` variant added
- [x] 21 unit tests: cost calculation, accumulation, multi-model, thresholds, milestones, status transitions, prefix matching

## Phase 3: Agent-initiated model switching ✅

- [x] Create `switch_model` tool in `src/tools/switch_model.rs`
- [x] Implement tool parameters: `role` (ModelRole string), `reason` (justification)
- [x] Validate switch request (disallow upgrade from cheap to expensive if over budget)
- [x] Emit `AgentEvent::ModelChange` with `reason: "agent_request"` via turn loop
- [x] Update agent's current model mid-turn via `ModelSwitchSlot` (Arc<Mutex<Option<String>>>)
- [x] Turn loop checks slot at top of each iteration, switches `active_model` for next LLM call
- [x] Agent syncs final model state after turn loop completes
- [x] 6 unit tests: switch to smol/slow, noop on same model, budget blocks upgrade, budget allows downgrade, is_upgrade ranking
- [ ] Add `switch_model` to system prompt tool descriptions
- [ ] Add agent examples: when to switch (task simpler/harder than expected)
- [ ] Unit tests: validation logic, rejected upgrades over budget
- [ ] Integration test: agent calls tool, model switches, turn continues

## Phase 4: TUI cost display ✅

- [x] Add `cost_tracker: Option<Arc<CostTracker>>` to App, read total_cost from it on UsageUpdate
- [x] Color-coded budget badge in status bar: green (ok), yellow (warning), red (exceeded), gray (no budget)
- [x] Budget badge shows remaining or exceeded amount
- [x] `StatusBarData.budget_status` field wired from App's cost tracker
- [x] Cost removed from trailing info string (now in dedicated badge)
- [ ] Add budget bar component (optional): visual progress toward limit — deferred
- [ ] Add cost breakdown panel (optional): per-model usage table — deferred
- [ ] Keybinding to toggle cost detail view (e.g., `C`) — deferred

## Phase 5: Cost inspection tool (agent self-awareness) ✅

- [x] Create `cost` tool in `src/tools/cost.rs`
- [x] Implement `breakdown` action: per-model table with tokens, cost, percentage
- [x] Implement `summary` action: one-line total and budget status
- [x] Implement `budget` action: remaining budget, projected capacity, status detail
- [x] Register `cost` tool module in `src/tools/mod.rs`
- [x] 5 unit tests: summary, breakdown, budget (no budget / with budget), unknown action

## Phase 6: Orchestration (experimental, disabled by default) ✅

- [x] `OrchestrationPlan` struct with pattern and ordered phases
- [x] `OrchestrationPattern` enum: ProposeValidate, PlanExecute, DraftReview
- [x] `OrchestrationPhase` struct: role, label, system_suffix
- [x] Builders: `propose_validate()`, `plan_execute()`, `draft_review()`
- [x] `detect_pattern()` — explicit hints ("plan and implement") + heuristic (high complexity + code generation)
- [x] 6 system prompt suffixes: Propose, Validate, Plan, Execute, Draft, Review
- [x] `enable_orchestration` flag on `RoutingPolicyConfig` (default: false)
- [x] `ModelSelectionResult.orchestration` field — routing policy populates when detected
- [x] `ComplexitySignals.prompt_text` field for pattern detection
- [x] `Agent::execute_orchestrated_turn()` — runs turn loop per phase with model switching
- [x] Phase-specific system prompts (base + suffix), model swap + ModelChange events
- [x] Later phases get shorter max_turns (10 vs 25) since they refine existing work
- [x] 10 unit tests: pattern detection (explicit, heuristic, low complexity), plan builders, display

## Phase 7: User hints and explicit role requests ✅

Already implemented in Phase 1:
- [x] `ModelRoleHint` enum: Explicit, Fast, Thorough (in `signals.rs`)
- [x] `parse_user_hint()` detects "use opus/haiku/sonnet", "quick answer", "think deeply"
- [x] User hints get highest priority in model selection (overrides complexity score)
- [x] Hard budget still overrides user hints (safety)
- [x] 6 tests: parsing, priority override, fast/thorough/explicit variants

## Phase 8: Configuration and persistence ✅

- [x] `routing: Option<RoutingPolicyConfig>` field on `Settings` (serde-enabled)
- [x] `cost_tracking: Option<CostTrackerConfig>` field on `Settings`
- [x] `--max-cost` / `--budget` CLI flag wired: creates cost tracker with 80% soft + hard limit
- [x] `--enable-routing` CLI flag: enables routing policy with defaults
- [x] Wired into interactive mode: routing policy + cost tracker from settings, cost tracker shared with TUI App
- [x] Wired into daemon mode: helper `wire_routing_from_settings()` applied at all 5 agent creation sites
- [x] Wired into json/print modes: same pattern, resolves pricing from global config dir

## Phase 9: Documentation and examples ✅

- [x] `docs/multi-model.md` — comprehensive guide (~820 lines): quick start, routing, roles, cost tracking, tools, orchestration, config reference, troubleshooting
- [x] README updated with multi-model section + CLI examples + link to docs
- [x] Config examples: routing thresholds, model roles, cost tracking, pricing override

## Phase 10: Testing and validation ✅

- [x] 28 integration tests in `src/routing/integration_tests.rs`
- [x] Full routing pipeline: simple→smol, complex→slow, medium→default
- [x] User hints: opus override, quick answer override
- [x] Budget: soft limit biases cheaper, hard limit forces smol, hard overrides user hint
- [x] Cost tracker: accumulation, budget status transitions (Ok→Warning→Exceeded), total matches summary, percentages sum to 100%
- [x] Model switching: smol succeeds, over-budget upgrade rejected, over-budget downgrade allowed, noop on current model
- [x] Orchestration: plan generation, explicit hints, high-complexity heuristic, disabled by default
- [x] Cost tool: summary/breakdown/budget actions
- [x] Performance: 1000 routing calls < 1s
- [x] End-to-end: routing + cost + switch interaction in single flow
- [x] Edge cases: disabled policy returns default, tool complexity weighting
