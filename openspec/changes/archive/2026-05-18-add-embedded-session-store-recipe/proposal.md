## Why

Clankers now has executable lego-style recipes for minimal turns, tool catalogs, and product-owned provider adapters. The remaining fuzzy integration seam for product embedding is host-owned session storage: products need a concrete example showing how to persist and restore enough conversation context without importing `clankers-db`, `clankers-session`, JSONL session files, daemon sessions, or TUI restore logic.

A small OpenSpec change should define this as a recipe/evidence slice, not as a new generic storage API. The goal is to prove that storage remains product-owned at the application edge while the green SDK crates still provide the engine/message/host contracts needed to resume context.

## What Changes

- **Session-store recipe**: Add a checked standalone recipe, tentatively `examples/embedded-session-store/`, that demonstrates host-owned persistence around the green embedded SDK crates.
- **Product DTO boundary**: Define product-owned session/message/turn receipt DTOs in the recipe and conversion helpers to/from `EngineMessage` or engine-run observations.
- **Resume proof**: Run a first turn, persist the transcript in an app-owned store, reload it, run a second turn with restored history, and assert the model request contains restored context plus the follow-up prompt.
- **Fail-closed behavior**: Cover missing/unknown session behavior with an explicit product error rather than silently creating hidden state or touching Clankers session files.
- **Acceptance/docs**: Add the recipe to the embedded SDK acceptance rail and document it as host-owned persistence, not a Clankers storage API.

## Capabilities

### Modified Capabilities

- `embedded-composition-kits.recipes`: Adds a storage/session recipe to the executable composition coverage.
- `embedded-composition-kits.acceptance-rail`: Extends one-command lego readiness to cover host-owned session persistence.
- `embeddable-runtime-stores.parity.in-memory-session`: Provides product-facing recipe evidence for the existing host-owned store parity requirement.

## Impact

- **Files**: `examples/embedded-session-store/`, `scripts/check-embedded-agent-sdk.sh`, `docs/src/tutorials/embedded-agent-sdk.md`, and generated API/example inventory if needed.
- **APIs**: No new reusable SDK trait is required for this change; the first slice should keep storage DTOs recipe-local until repeated product usage justifies promotion.
- **Dependencies**: The recipe MUST depend only on green embedded SDK crates and small utility crates. It MUST NOT depend on `clankers-db`, `clankers-session`, `clankers-agent`, `clankers-controller`, daemon/TUI/provider/router/plugin/Matrix/iroh crates, or OAuth/session-file machinery.
- **Testing**: Verify with the new recipe, dependency-denylist coverage, `scripts/check-embedded-agent-sdk.sh`, `git diff --check`, and the smallest relevant Cargo check.
