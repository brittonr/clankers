## MODIFIED Requirements

### Requirement: Executable composition recipes [r[embedded-composition-kits.recipes]]

The system MUST provide checked executable recipes that demonstrate supported lego-style compositions, including product-owned model, tool, and session-storage seams.

#### Scenario: Recipes cover positive and negative paths [r[embedded-composition-kits.recipes.coverage]]

- GIVEN embedded composition recipes are checked into `examples/`
- WHEN `scripts/check-embedded-agent-sdk.sh` runs
- THEN it MUST compile/run at least a minimal recipe, a tool-enabled recipe, a product-owned provider-adapter recipe, and a negative/fail-closed catalog or capability-policy recipe
- THEN recipe dependency graphs MUST be checked for forbidden shell/runtime dependencies

#### Scenario: Session-store recipe preserves restored context [r[embedded-composition-kits.recipes.session-store-restores-context]]

- GIVEN a standalone embedded session-store recipe uses product-owned session/message DTOs and an app-owned in-memory store
- WHEN the recipe runs one embedded turn, persists the resulting transcript, reloads the session, and runs a follow-up turn
- THEN the follow-up `EngineModelRequest` MUST include the restored prior user/assistant context and the new follow-up prompt in deterministic order
- THEN the recipe MUST preserve the supplied `session_id` through persistence, reload, and model-host request observation

#### Scenario: Session-store recipe fails closed for missing sessions [r[embedded-composition-kits.recipes.session-store-missing-session]]

- GIVEN the product-owned store has no session for a requested id
- WHEN the recipe attempts to restore that session for a follow-up turn
- THEN it MUST return an explicit product-owned missing-session error
- THEN it MUST NOT silently create a replacement session, read Clankers JSONL session files, open `clankers-db`, contact a daemon, or depend on TUI/session restore logic

#### Scenario: Green/yellow/red crate guidance is generated or checked [r[embedded-composition-kits.recipes.crate-guidance]]

- GIVEN product docs describe which Clankers crates are appropriate for product embeddings
- WHEN the embedded SDK acceptance rail runs
- THEN docs MUST classify generic SDK crates as green, app-edge integration crates as yellow, and shell/internal crates as red for generic embedding
- THEN the classification MUST state that product-owned storage/session DTOs are a yellow app-edge concern unless and until a separate OpenSpec promotes a reusable storage API
- THEN the classification MUST be checked against the actual workspace crate list or an explicit reviewed inventory

### Requirement: Embedded composition acceptance rail [r[embedded-composition-kits.acceptance-rail]]

The system MUST extend the existing embedded SDK acceptance command so lego-style composition claims are verified before readiness is claimed.

#### Scenario: One command verifies lego readiness [r[embedded-composition-kits.acceptance-rail.one-command]]

- GIVEN a developer changes adapter bricks, kits, catalogs, capability packs, provider/session recipes, or embedded SDK docs
- WHEN `scripts/check-embedded-agent-sdk.sh` runs
- THEN it MUST verify API inventory freshness, dependency denylist coverage, source-boundary checks, executable recipes, catalog negative cases, capability-pack snapshots, host-owned session-store recipe behavior, and focused engine/host/tool parity tests
- THEN failure MUST identify the violated lego-boundary rule with enough detail to fix the offending dependency, source token, catalog field, session-store assertion, or recipe assertion
