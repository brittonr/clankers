## Phase 1: Recipe scope and DTO seam

- [x] [serial] Define the standalone `examples/embedded-session-store/` recipe with product-owned `ProductSession`/`ProductMessage` DTOs and an in-memory store. [covers=embedded-composition-kits.recipes.session-store-restores-context] [evidence=examples/embedded-session-store/src/main.rs]
- [x] [depends:recipe-dto] Add conversion helpers from product transcript entries into engine history without importing `clankers-db`, `clankers-session`, daemon, TUI, provider, router, or controller crates. [covers=embedded-composition-kits.recipes.session-store-restores-context] [evidence=scripts/check-embedded-sdk-deps.rs]

## Phase 2: Executable restore/fail-closed behavior

- [x] [depends:recipe-dto] Implement a positive executable scenario: create session, run first turn, persist transcript, reload, run follow-up turn, and assert recorded `EngineModelRequest` history contains restored prior context plus the follow-up prompt in deterministic order. [covers=embedded-composition-kits.recipes.session-store-restores-context] [evidence=RUSTC_WRAPPER= cargo run --locked --manifest-path examples/embedded-session-store/Cargo.toml]
- [x] [parallel] Implement a missing-session/fail-closed scenario that returns an explicit product-owned error and proves no hidden replacement session path is used. [covers=embedded-composition-kits.recipes.session-store-missing-session] [evidence=RUSTC_WRAPPER= cargo run --locked --manifest-path examples/embedded-session-store/Cargo.toml]

## Phase 3: Acceptance/docs

- [x] [depends:recipe-runs] Add the new recipe to `scripts/check-embedded-agent-sdk.sh` and extend dependency-denylist coverage if the existing checks do not cover standalone recipes. [covers=embedded-composition-kits.acceptance-rail.one-command] [evidence=scripts/check-embedded-agent-sdk.sh]
- [x] [parallel] Update embedded SDK docs and API/example inventory so host-owned session persistence is documented as an app-edge/yellow concern and the recipe is discoverable. [covers=embedded-composition-kits.recipes.crate-guidance] [evidence=docs/src/tutorials/embedded-agent-sdk.md]

## Phase 4: Verification and archive

- [x] [depends:acceptance-docs] Run focused recipe checks, `scripts/check-embedded-agent-sdk.sh`, `git diff --check`, and a relevant Cargo check. [covers=embedded-composition-kits.acceptance-rail.one-command] [evidence=scripts/check-embedded-agent-sdk.sh]
- [x] [depends:verification] Promote/sync the `embedded-composition-kits` spec delta and archive this change after all implementation tasks are complete. [evidence=openspec validate add-embedded-session-store-recipe --strict --json]
