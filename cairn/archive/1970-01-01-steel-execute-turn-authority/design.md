# Design: Steel Execute Turn Authority

## Context

`steel.host.plan_turn` remains a planning seam. In default rollout, an authorized Steel plan selects the Steel execution adapter, which delegates actual provider/tool work to `clankers-engine-host::run_engine_turn(...)`. The previous execution receipt was emitted only after that host runner returned. This change makes the execution branch cross an explicit Rust-owned authority check before host-runner effects.

## Decisions

### 1. Execution is a separate host-action contract

**Choice:** The default Steel orchestration profile now lists both `steel.host.plan_turn` and `steel.host.execute_turn`. Planning keeps `turn-planning` plus `clankers/steel/orchestrate.plan_turn`; execution requires `turn-execution` plus `clankers/steel/orchestrate.execute_turn`.

**Rationale:** A planning grant should not implicitly authorize the selected execution branch. Separate profile entries keep future seam expansion reviewable and make disabled-action policy able to block execution without pretending the planner failed.

### 2. Runtime owns the execution DTO and receipt

**Choice:** `clankers-runtime::authorize_steel_turn_execution(...)` accepts `SteelTurnExecutionInput`, builds a `DynamicRuntimeActionEnvelope` for `steel.host.execute_turn`, and returns `SteelTurnExecutionReceipt` with the dynamic authorization receipt hash and redaction metadata.

**Rationale:** The runtime already owns Steel orchestration DTOs and dynamic-runtime authorization. Keeping the execution DTO there avoids leaking interpreter details into agent/controller/daemon shells and gives checkers a stable schema surface.

### 3. Agent adapter blocks before host runner on denial

**Choice:** `turn/steel_execution.rs::run_steel_selected_engine_turn(...)` authorizes execution before incrementing the test hook or calling `run_engine_turn(...)`. Denials emit a daemon-visible `steel.host.execute_turn` receipt with `status=Denied`, authority status/reason, input hash, and authority receipt hash, then return an agent error.

**Rationale:** This proves the provider/tool runner is not used to mask missing execution authority. The same receipt path also keeps allowed execution observable with `authority_status=Allowed` and `authority_reason=Ready`.

## Verification Plan

- Runtime unit tests cover allowed execution authority plus missing-UCAN/disabled-action denial.
- Real turn-loop test asserts default Steel execution emits authority fields and still omits prompt text.
- Embedded controller smoke asserts daemon-visible allowed execution authority and a missing-execution-authority denial before provider calls.
- `scripts/check-steel-execute-turn-authority.rs` checks DTO, profile, docs, and smoke markers and writes a deterministic receipt.

## Risks / Trade-offs

- Existing explicit test profiles must list the new execution host action even when they run in comparison mode. This is intentional because the reviewed profile surface now owns both default planning and selected execution authority.
- The execution receipt remains metadata-only; it proves authorization and host-runner return status, not raw provider/tool payloads.
