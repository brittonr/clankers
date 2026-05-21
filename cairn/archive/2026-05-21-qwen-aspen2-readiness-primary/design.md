## Context

The repository already has a live qwen/aspen2 readiness seam:

- `./scripts/test-harness.sh live aspen2-qwen36`
- `tests/readiness_opt_in.rs::readiness_live_local_model_aspen2_qwen36_nextest_opt_in`
- `tests/aspen2_qwen36_integration.rs`
- default endpoint/model `http://aspen2:13305/v1` and `user.Qwen3.6-35B-A3B`

The missing piece is policy clarity: for this class of testing/dogfood/readiness work, qwen on aspen2 is the primary live test model path. Prior evidence pages can still accurately describe old runs where Codex was primary and qwen/aspen2 was fallback; new readiness guidance should not require future operators to infer the preferred model from chat history.

## Decisions

### 1. Make qwen/aspen2 the primary live testing model in docs and tests

**Choice:** Update release-readiness docs and a doc regression test to state that qwen on aspen2 is the primary live testing/readiness model for this workstream.

**Rationale:** This is the smallest durable rail that changes future operator behavior without pretending the pure baseline gates need live credentials or network access.

### 2. Reuse the existing live harness seam

**Choice:** Keep the existing selector and Rust integration test names (`aspen2-qwen36`) instead of renaming the harness surface.

**Rationale:** The existing seam already exercises the routed provider path and streaming/reasoning-or-text contract. Renaming would add churn without improving evidence quality.

### 3. Separate new policy from old evidence

**Choice:** Do not rewrite older release evidence that truthfully recorded Codex-primary runs. Preserve a fresh qwen/aspen2 receipt for this slice and document the transition point.

**Rationale:** Evidence pages must remain historically accurate. The change establishes forward policy and fresh verification rather than revising past operator receipts.

## Risks / Trade-offs

- The qwen/aspen2 live rail is host-dependent. If the endpoint is unavailable, the slice can only preserve an explicit unavailable/prerequisite receipt and must not claim live model verification.
- The harness still carries the historical `qwen36` selector name. This avoids churn but requires docs to be explicit that it is the primary live testing model for current readiness work.
