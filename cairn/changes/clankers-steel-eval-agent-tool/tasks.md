## Phase 1: Contract and policy

- [ ] [serial] T1. Add the `steel-eval-agent-tool` delta spec with request, registration, authority, receipt, and verification requirements. [covers=r[steel-eval-agent-tool.request-contract],r[steel-eval-agent-tool.registration-policy],r[steel-eval-agent-tool.authority-boundary],r[steel-eval-agent-tool.receipt-contract],r[steel-eval-agent-tool.verification-contract]]
- [ ] [serial] T2. Define the reviewed settings/profile switch that enables `steel_eval`, including fail-closed behavior when Steel support or the selected profile is unavailable. [covers=r[steel-eval-agent-tool.registration-policy.enabled],r[steel-eval-agent-tool.registration-policy.unavailable]]

## Phase 2: Rust tool surface

- [ ] [serial] T3. Implement the built-in `steel_eval` tool as a thin shell over the existing Steel runtime wrapper, with typed request validation and named profile selection. [covers=r[steel-eval-agent-tool.request-contract.wrapper-delegation],r[steel-eval-agent-tool.request-contract.profile-selection]]
- [ ] [serial] T4. Enforce deny-by-default host authority, disabled-tool checks, source/output/host-call/execution budgets, and no ambient fallback authority. [covers=r[steel-eval-agent-tool.authority-boundary.pure-default],r[steel-eval-agent-tool.authority-boundary.denied-host-function],r[steel-eval-agent-tool.authority-boundary.no-ambient-fallback]]
- [ ] [parallel] T5. Wire discovery and disabled-tool parity for standalone, daemon, local attach, and remote attach tool-list paths. [covers=r[steel-eval-agent-tool.registration-policy.tool-list-parity],r[steel-eval-agent-tool.registration-policy.disabled-parity]]

## Phase 3: Receipts and verification

- [ ] [serial] T6. Emit deterministic safe receipts for success, denial, unavailable runtime/profile, resource limits, and redaction outcomes. [covers=r[steel-eval-agent-tool.receipt-contract.success],r[steel-eval-agent-tool.receipt-contract.failure],r[steel-eval-agent-tool.receipt-contract.redaction]]
- [ ] [parallel] T7. Add positive and negative fixtures for pure eval, wrapper delegation, disabled tool/profile, unknown host function, budget limits, and redaction. [covers=r[steel-eval-agent-tool.verification-contract.positive-fixture],r[steel-eval-agent-tool.verification-contract.negative-fixture],r[steel-eval-agent-tool.verification-contract.redaction-fixture]]
- [ ] [parallel] T8. Add registry/daemon/attach parity tests proving `steel_eval` exposure and disabled-tool behavior are consistent across supported contexts. [covers=r[steel-eval-agent-tool.verification-contract.parity-fixture]]
- [ ] [serial] T9. Run focused Rust checks plus Cairn validate/proposal/design/tasks gates before implementation closeout. [covers=r[steel-eval-agent-tool.verification-contract.closeout]]
