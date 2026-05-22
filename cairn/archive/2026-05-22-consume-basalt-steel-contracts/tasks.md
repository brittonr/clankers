# Tasks: consume-basalt-steel-contracts

## Implementation

- [x] [serial] I1: Add a narrow Basalt DTO bridge for `steel.host.plan_turn` request construction. [r[steel-default-orchestration.basalt-contract-bridge]]
- [x] [serial] I2: Validate the Basalt Steel evaluation request before invoking the Steel runtime; fail closed or policy-fallback without granting ambient authority. [r[steel-default-orchestration.basalt-contract-bridge]] [r[steel-default-orchestration.basalt-contract-bridge.fail-closed]]
- [x] [serial] I3: Bind successful Steel planning receipts to Basalt request/receipt hashes and expose them in Clankers orchestration receipts. [r[steel-default-orchestration.basalt-contract-bridge.receipts]]

## Tests

- [x] [serial] T1: Add positive runtime/agent coverage showing authorized Steel turn planning carries Basalt request/receipt evidence. [r[steel-default-orchestration.basalt-contract-bridge.agent-turn]]
- [x] [serial] T2: Add negative coverage showing missing UCAN authority invalidates the Basalt allow receipt and keeps execution fail-closed/Rust-owned. [r[steel-default-orchestration.basalt-contract-bridge.fail-closed]]

## Verification

- [x] [serial] V1: Run focused Clankers runtime and agent Steel-planning tests. [r[steel-default-orchestration.basalt-contract-bridge.agent-turn]]
- [x] [serial] V2: Run focused clippy/format/Cairn validation before archiving. [r[steel-default-orchestration.basalt-contract-bridge.closeout]]
