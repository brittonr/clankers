## Context

Current Clankers Steel state:

- `steel_eval` is already published by default as a pure, no-host-call agent tool.
- `clankers steel status|eval|run` already use the Rust-owned Steel wrapper.
- real agent turns can already call `steel.host.plan_turn` when `steelTurnPlanning` settings provide a profile/script, capabilities, and UCAN ability.
- `SteelTurnPlanningSettings::default()` currently disables turn planning, so ordinary sessions never reach the Steel planner unless settings opt in.

This change targets turn planning only. It does not change `steel_eval` and does not make Steel an authority boundary.

## Decisions

### 1. Missing config means bundled reviewed default

**Choice:** Treat missing `steelTurnPlanning` settings as a request for the bundled reviewed `steel.host.plan_turn` profile/script. Keep `steelTurnPlanning.enabled = false` as the explicit opt-out.

**Rationale:** User-facing default behavior should match the accepted Steel default-orchestration capability. The kill switch remains simple and local.

**Implementation note:** The implementation may either make `SteelTurnPlanningSettings::default()` contain the bundled defaults, or introduce a shell-level resolver that distinguishes absent/explicit-disabled config before calling the existing activation helper. The resolver must be covered by tests so absent config, explicit disabled config, and explicit override config do not drift.

### 2. Default authority is seam-limited and session-scoped

**Choice:** Default activation may supply only the capabilities/abilities required for `steel.host.plan_turn` on the current session turn resource. It must not grant mutation, filesystem, shell, network, provider, credential, daemon, TUI, native-tool, or arbitrary host-function authority.

**Rationale:** `steel.host.plan_turn` proposes typed plan data; Rust remains the authority for all effects. A default planner needs enough authority to plan, not enough authority to act.

### 3. Bundled profile/script stay checked-in and hash-bound

**Choice:** Use `policy/steel-default-orchestration/orchestration-profile.json` and `policy/steel-default-orchestration/scripts/default-plan-turn.scm` as the default artifacts, with BLAKE3 hash evidence in receipts.

**Rationale:** The accepted Steel specs require reviewed policy/script material. Hash-bound receipts let reviewers confirm which source-backed profile authored a decision without exposing raw prompts or script bodies.

### 4. Fallback remains explicit

**Choice:** The default bundled profile keeps `fallback_mode = rust_native` unless a reviewed policy changes it. Invalid activation fails closed before Steel runs, then follows the reviewed fallback/block policy.

**Rationale:** Making Steel default should improve orchestration coverage without making ordinary sessions brittle or granting broader fallback authority.

## Risks / Trade-offs

- **Compatibility:** Ordinary sessions will begin emitting Steel turn-planning receipts. Mitigation: keep receipts redacted and document the opt-out.
- **Authority confusion:** Default capabilities could be mistaken for tool/provider authority. Mitigation: restrict defaults to `steel.host.plan_turn` and assert no ambient authority in tests/checkers.
- **Config ambiguity:** Existing default settings cannot currently distinguish absent config from explicit disable. Mitigation: make that distinction explicit in implementation and tests.
- **Profile freshness:** Checked-in profile/script hash behavior must stay deterministic. Mitigation: add focused default-profile/hash tests and update existing Steel checker receipts.
