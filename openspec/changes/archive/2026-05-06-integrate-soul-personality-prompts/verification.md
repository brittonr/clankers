# Verification: integrate-soul-personality-prompts

## Inventory

Touched seams for SOUL/personality prompt integration:

- `crates/clankers-agent/src/system_prompt.rs` — prompt resource discovery, SOUL/personality section assembly, local SOUL/preset policy, safe metadata, and unit tests.
- `src/soul_personality.rs` — existing CLI/tool validation policy surface; retained as the non-mutating validate/status receipt seam.
- `src/tools/soul_personality.rs` — existing Specialty tool receipt surface; focused tests rerun.
- `README.md` — documented SOUL prompt assembly, precedence, env controls, and metadata boundary.
- `docs/src/reference/config.md` — documented local SOUL/preset discovery and env controls.
- `docs/src/reference/request-lifecycle.md` — documented prompt assembly order and safe metadata boundary.

## Behavior verified

- Local `.clankers/SOUL.md`, project-root `SOUL.md`, or global `~/.clankers/agent/SOUL.md` is discovered into prompt resources when SOUL integration is not disabled.
- `CLANKERS_DISABLE_SOUL_PERSONALITY=1` omits SOUL and preset sections while recording disabled metadata.
- `CLANKERS_PERSONALITY_PRESET=<safe-id>` includes `.clankers/personality/<safe-id>.md` or global personality prompt files.
- Unsafe personality ids record safe error metadata and do not include raw input.
- Prompt assembly order is SYSTEM/APPEND_SYSTEM, SOUL/personality, AGENTS.md/CLAUDE.md, context, specs, skills, learning guidance, settings suffix.
- SOUL/personality metadata records kind/status/precedence/preset id/path hash/byte count/error kind without raw persona contents or full paths.
- Existing CLI/tool validation remains local-policy-only and rejects remote/command sources.

## Commands

Captured from commit base `dae903b5` at `2026-05-06T21:27:25Z`.

- `cargo fmt --check` — passed.
- `CARGO_TARGET_DIR=target cargo test -p clankers-agent system_prompt -- --nocapture` — passed: 38 passed, 0 failed.
- `CARGO_TARGET_DIR=target cargo test --lib soul_personality -- --nocapture` — passed: 7 passed, 0 failed.
- `CARGO_TARGET_DIR=target cargo test --lib tools::soul_personality -- --nocapture` — passed: 2 passed, 0 failed.
- `openspec validate integrate-soul-personality-prompts --strict` — passed.
