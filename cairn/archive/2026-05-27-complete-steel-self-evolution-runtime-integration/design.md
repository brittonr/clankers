# Design

## Correction summary

Prior archived artifacts described a complete self-evolution path, but code only implemented validation cores. This change makes the correction explicit and adds runtime evidence. Completion requires both code seams and focused check outputs.

## Decisions

### Decision: turn runtime loads repo-local evolution packs

`Agent::run_turn_loop` and the orchestrated turn path call `load_repo_evolution_pack(...)` before planning. Absent packs remain inactive. Present packs emit only safe receipt metadata through `AgentEvent::SystemMessage`; they do not execute unchecked Steel source.

### Decision: higher-order contracts wrap host calls

`SteelRepoEvolutionPack` gains `host_contracts`. Every `allowed_host_calls` entry must be covered by a contract whose mode is `higher_order` and whose pre/postconditions are non-empty. Plan evaluation also rejects host calls lacking a contract before any host effect.

### Decision: Nickel remains source of policy shape

Repo packs include `.clankers/steel/evolution-profile.ncl` plus exported `.json`. Runtime checks validate required Nickel contract markers and exported typed data. The repository-local profile demonstrates the contract shape and the focused checker hashes both source and export.

### Decision: orchestration mutation has real isolated staging

`stage_orchestration_patch_to_directory(...)` validates the typed proposal, verifies payload targets exactly match the validated target list, writes only below the supplied staging root, and then builds the staged receipt. `promote_staged_orchestration_pack_to_directory(...)` then hash-checks live and staged target sets, backs up live files, and copies staged files to live. `rollback_orchestration_pack_to_directory(...)` restores from backup only when current and backup hashes match the receipt. Failed gates and path/payload errors leave live files untouched.

### Decision: restored requirements stay canonical

The self-mutation policy again includes apply-through-Rust, raw-write-denied, preflight, safe-receipt, failed-verification, guarded-rollback, and positive/negative fixture scenarios. Tasks cite those restored requirements directly.

## Verification plan

Focused verification is enough for this correction: runtime crate tests for repo-evolution and orchestration mutation, both Steel checker scripts, a clankers-agent compile/test slice that covers the turn-path call site, docs build, Cairn validate, and whitespace diff checks.
