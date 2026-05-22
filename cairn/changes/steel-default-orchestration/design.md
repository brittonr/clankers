# Design: Steel Default Orchestration

## Architecture split

The design keeps the established split intact:

```text
Nickel = declared orchestration policy/profile/script bindings
UCAN   = runtime delegated authority for host actions
Rust   = enforcement, I/O, receipts, verification, fallback, rollback
Steel  = trusted orchestration/request/planning logic
Wasm   = untrusted/tool execution boundary
```

Steel becomes the default *planner* only after policy says so. It returns typed data to Rust. Rust decides whether a plan is acceptable, performs any effectful work, and records receipts.

## Seams to preserve

### Runtime wrapper seam

All Steel execution must pass through `clankers-runtime::steel_runtime` or a thin adapter over it. Product shells must not import or construct interpreter internals.

### Orchestration adapter seam

Introduce a focused Rust adapter, conceptually:

```rust
trait OrchestrationPlanner {
    fn plan_turn(&self, input: TurnPlanningInput) -> OrchestrationPlanReceipt;
}
```

A `RustNativePlanner` preserves current behavior. A `SteelPlanner` loads configured Steel source through the runtime wrapper and converts the returned value into a typed `OrchestrationPlan`.

Callers depend on the trait/adapter, not on Steel. This keeps controller/agent/TUI/daemon code insulated from interpreter details.

### Typed plan seam

Steel output must be parsed as a typed plan, for example:

- selected host action/action envelope,
- tool candidate ordering,
- routing hint,
- memory/summary/compaction intent,
- mutation proposal intent,
- no-op/defer/fallback decision.

Plans must carry stable schema versions, profile/script hashes, declared required capabilities, and redaction class. Free-form textual script output is not an executable plan.

### Authorization seam

Every effectful item in a Steel plan must cross existing Rust checks:

- dynamic-runtime action envelope authorization,
- session capability and disabled-tool policy,
- Nickel policy/profile checks,
- UCAN-style ability/resource checks where applicable,
- provider/router request-shape ownership,
- mutation preflight/apply/rollback shells for mutation actions.

Steel can request; Rust authorizes and applies.

### Fallback seam

Steel orchestration is policy-selectable and kill-switchable. If script loading, script evaluation, typed-plan parsing, profile validation, or receipt validation fails, the adapter must return a typed fallback/blocked receipt and route to the Rust-native planner when policy allows fallback.

The fallback must not retry Steel under a less restrictive profile.

## Rollout stages

1. **Profile and policy rail**: Add Nickel orchestration profile declaring default/disabled state, scripts, budgets, host functions, fallback mode, and receipt requirements.
2. **Pure plan DTOs**: Add Rust DTOs for `TurnPlanningInput`, `OrchestrationPlan`, `OrchestrationDecision`, and `OrchestrationPlanReceipt` with no I/O.
3. **Fake Steel planner fixture**: Use deterministic fake Steel output to prove typed parsing, authorization routing, and denial behavior before real interpreter wiring.
4. **Adapter wiring for one low-risk seam**: Enable Steel planning for one reviewed decision such as tool-candidate ordering or host-action proposal, while Rust-native output remains the comparison/fallback oracle.
5. **Receipt comparison rail**: Record Steel-vs-Rust-native decision receipts for dogfood review before expanding default coverage.
6. **Default-on profile**: Only after parity evidence, flip the reviewed profile to default for the selected seam.

## Policy shape

Nickel should declare:

- profile name and version,
- enabled/default state,
- allowed planning seams,
- script source/resource identifiers,
- script hash requirements,
- runtime budget profile,
- allowed host functions/actions,
- required UCAN/session capabilities,
- fallback mode,
- receipt destination and redaction policy,
- rollout stage and audit metadata.

## Security and safety model

- Steel has no ambient authority.
- Steel does not become a sandbox boundary.
- Steel-produced plans are untrusted until Rust validates schema, policy, authority, budget, and receipts.
- Host effects are explicit and typed.
- Provider calls remain owned by provider/router adapters.
- Session/daemon/TUI state mutation remains owned by Rust shells.
- Mutation actions remain behind the existing Steel self-mutation policy and Rust preflight/apply/rollback seams.

## Verification approach

Verification should include:

- positive fake Steel planner fixture producing a typed no-effect plan,
- positive fixture producing an allowed dynamic-runtime envelope that Rust authorizes without immediate side effects,
- negative fixture for unknown host action,
- negative fixture for script hot reload trying to add host authority,
- negative fixture for direct provider/credential/daemon/TUI/native-tool access,
- fallback fixture when script parse/eval/plan parsing fails,
- checker proving CLI/daemon/TUI/controller/provider modules do not import interpreter internals directly,
- docs that explicitly avoid sandbox or authority overclaims.

## Open questions

- Which first decision seam should be promoted: tool ordering, action proposal, compaction planning, or routing hints?
- Should default-on be scoped per session, per project, or per profile?
- How should comparison receipts be surfaced in the TUI without adding noise?
