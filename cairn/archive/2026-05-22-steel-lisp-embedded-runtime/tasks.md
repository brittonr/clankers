## Phase 1: Runtime wrapper and CLI surfaces

- [x] [serial] I1: Add the Steel runtime wrapper with Clankers-owned request/response DTOs, named evaluation profiles, and structured receipts [r[steel-lisp-runtime.wrapper-owned-evaluation]] [r[steel-lisp-runtime.deterministic-limits-and-redaction]]
- [x] [serial] I2: Add `clankers steel status`, `clankers steel eval`, and `clankers steel run` through the wrapper without direct shell imports of Steel interpreter internals [r[steel-lisp-runtime.wrapper-owned-evaluation.cli-wrapper]] [r[steel-lisp-runtime.explicit-surfaces.status]]
- [x] [parallel] I3: Add host-function registration and fake-host test seams that require explicit capability/disabled-tool approval before any host effect executes [r[steel-lisp-runtime.capability-gated-host-effects]]
- [x] [parallel] I4: Implement resource-limit and redaction behavior for source/output/host-call/execution-budget failures with stable issue codes [r[steel-lisp-runtime.deterministic-limits-and-redaction.output-limit]] [r[steel-lisp-runtime.deterministic-limits-and-redaction.execution-budget]]

## Phase 2: Optional agent/tool integration

- [x] [serial] I5: Agent-visible Steel eval remains disabled in this drain; the runtime wrapper exposes shared DTOs/receipts and host-function authorization so a future `steel_eval` tool can reuse the same checks without creating a separate interpreter path [r[steel-lisp-runtime.explicit-surfaces.agent-tool-shares-runtime]] [r[steel-lisp-runtime.capability-gated-host-effects.denied-host-function]]
- [x] [serial] I6: Add architecture or compile checks proving root CLI, daemon, TUI, attach, provider, controller, and embeddable-engine shell modules call wrapper/adapters instead of constructing Steel interpreter internals directly [r[steel-lisp-runtime.wrapper-owned-evaluation.no-shell-interpreter-leak]] [r[steel-lisp-runtime.implementation-constraints.dependency-isolation]]
- [x] [serial] I7: Encode the implementation constraints in docs/status/receipts and rails: zero ambient authority, named profile budgets, no sandbox overclaim, no live fallback, and credential-free deterministic fixtures [r[steel-lisp-runtime.implementation-constraints]]

## Phase 3: Verification and docs

- [x] [parallel] V1: Add deterministic positive fixtures for pure Steel evaluation and an approved fake host function, including repeated-run receipt comparison [r[steel-lisp-runtime.verification-contracts.positive-fixture]]
- [x] [parallel] V2: Add negative fixtures for unknown/unauthorized host functions, zero ambient authority, profile-owned budgets, no-sandbox-overclaim output, no live fallback, output truncation, and execution-budget failure [r[steel-lisp-runtime.verification-contracts.negative-authority-fixture]] [r[steel-lisp-runtime.deterministic-limits-and-redaction]] [r[steel-lisp-runtime.implementation-constraints]]
- [x] [serial] V3: Verify CLI status/eval/run behavior, optional daemon `ToolList` and disabled-tool rebuild parity, Cairn validate/gates, formatting, and the smallest relevant Clankers Rust checks before sync/archive [r[steel-lisp-runtime.explicit-surfaces]] [r[steel-lisp-runtime.verification-contracts]]
