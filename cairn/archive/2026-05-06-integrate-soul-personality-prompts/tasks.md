## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `integrate-soul-personality-prompts`.
- [x] Validate the OpenSpec package with `openspec validate integrate-soul-personality-prompts --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory current `soul-personality-system` code/docs seams and record the exact files to touch. Evidence: `verification.md` lists `crates/clankers-agent/src/system_prompt.rs`, existing CLI/tool validation seams, README, config docs, and request lifecycle docs.
- [x] Add typed policy/config/request/receipt models with unit tests. Evidence: `SoulPromptAssembly`, `SoulPromptMetadata`, `SoulPromptSourceKind`, and `SoulPromptStatus` model safe prompt-inclusion metadata; existing `SoulValidation` remains the CLI/tool receipt model; unit tests cover included, disabled, preset, and invalid-preset metadata.
- [x] Implement the first runtime/adapter slice behind deterministic fake tests. Evidence: `load_soul_personality(...)` discovers local SOUL.md and local preset files under temp dirs in deterministic `clankers-agent` tests.
- [x] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy. Evidence: SOUL/personality is part of normal `discover_resources(...)` and `assemble_system_prompt(...)`; validation CLI/tool surfaces remain local-policy-only and non-mutating.
- [x] Update README and relevant docs for supported behavior, non-goals, and safety policy. Evidence: README, `docs/src/reference/config.md`, and `docs/src/reference/request-lifecycle.md` document discovery, env controls, precedence, non-goals, and metadata boundaries.

## Phase 3: Verification and Closeout

- [x] Run targeted package/integration checks for the touched modules. Evidence: `cargo test -p clankers-agent system_prompt`, `cargo test --lib soul_personality`, and `cargo test --lib tools::soul_personality` passed.
- [x] Run `cargo check --tests` for affected crates. Evidence: `CARGO_TARGET_DIR=target cargo check --tests` passed.
- [x] Run `git diff --check`. Evidence: `git diff --check` passed.
- [x] Sync the delta spec into the canonical `soul-personality-system` spec and archive the change after implementation tasks complete. Evidence: `openspec archive integrate-soul-personality-prompts --yes` updated the canonical spec and archived the change as `2026-05-06-integrate-soul-personality-prompts`.
