# Design: Harden Steel Substrate Checker Paths

## Context

The Steel substrate checker is part of the durable evidence surface for `steel-tool-plugin-substrate`. It verifies runtime DTO markers, agent adapter markers, settings activation markers, executor-kind tags, lifecycle task traceability, and canonical spec markers before writing `target/steel-tool-plugin-substrate/receipt.json`.

After archive, the active change directory is intentionally absent. A durable checker must therefore not require `cairn/changes/steel-tool-plugin-substrate/...` to exist.

## Decisions

### 1. Resolve active paths before archived/canonical fallbacks

**Choice:** Keep active paths for future in-progress changes, but add helper functions that choose:

- active `cairn/changes/steel-tool-plugin-substrate/tasks.md` when present;
- otherwise archived `cairn/archive/1970-01-01-steel-tool-plugin-substrate/tasks.md`;
- active delta spec when present;
- otherwise canonical `cairn/specs/steel-tool-plugin-substrate/spec.md`.

**Rationale:** This preserves future active-change validation while making the checker stable on the current archived repository state.

### 2. Hash the resolved paths in the receipt

**Choice:** Use the resolved task/spec paths in the `hashed_artifacts` list.

**Rationale:** The receipt should prove exactly which lifecycle artifacts were validated. Hashing stale active paths would be impossible; hashing hard-coded fallback paths would hide future active-change validation.

## Risks / Trade-offs

- If a future active change intentionally changes the substrate spec markers, the checker will validate the active delta first. That is desirable for active work, but it means marker changes still need coordinated checker updates.
- The archived task path remains fixed to the archived change that supplied the original substrate evidence.
