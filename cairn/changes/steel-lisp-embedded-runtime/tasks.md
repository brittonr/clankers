## Phase 1: Runtime wrapper and CLI surfaces

- [ ] [serial] I1: Add the Steel runtime wrapper with Clankers-owned request/response DTOs, named evaluation profiles, and structured receipts [r[steel-lisp-runtime.wrapper-owned-evaluation]] [r[steel-lisp-runtime.deterministic-limits-and-redaction]]
- [ ] [serial] I2: Add `clankers steel status`, `clankers steel eval`, and `clankers steel run` through the wrapper without direct shell imports of Steel interpreter internals [r[steel-lisp-runtime.wrapper-owned-evaluation.cli-wrapper]] [r[steel-lisp-runtime.explicit-surfaces.status]]
- [ ] [parallel] I3: Add host-function registration and fake-host test seams that require explicit capability/disabled-tool approval before any host effect executes [r[steel-lisp-runtime.capability-gated-host-effects]]
- [ ] [parallel] I4: Implement resource-limit and redaction behavior for source/output/host-call/execution-budget failures with stable issue codes [r[steel-lisp-runtime.deterministic-limits-and-redaction.output-limit]] [r[steel-lisp-runtime.deterministic-limits-and-redaction.execution-budget]]

## Phase 2: Optional agent/tool integration

- [ ] [serial] I5: If agent-visible Steel evaluation is enabled, register a `steel_eval` tool that reuses the same runtime wrapper, capability checks, disabled-tool policy, and receipts in standalone and daemon sessions [r[steel-lisp-runtime.explicit-surfaces.agent-tool-shares-runtime]] [r[steel-lisp-runtime.capability-gated-host-effects.denied-host-function]]
- [ ] [serial] I6: Add architecture or compile checks proving root CLI, daemon, TUI, attach, and provider modules call wrapper/adapters instead of constructing Steel interpreter internals directly [r[steel-lisp-runtime.wrapper-owned-evaluation.no-shell-interpreter-leak]]

## Phase 3: Verification and docs

- [ ] [parallel] V1: Add deterministic positive fixtures for pure Steel evaluation and an approved fake host function, including repeated-run receipt comparison [r[steel-lisp-runtime.verification-contracts.positive-fixture]]
- [ ] [parallel] V2: Add negative fixtures for unknown/unauthorized host functions, ambient authority attempts, output truncation, and execution-budget failure [r[steel-lisp-runtime.verification-contracts.negative-authority-fixture]] [r[steel-lisp-runtime.deterministic-limits-and-redaction]]
- [ ] [serial] V3: Verify CLI status/eval/run behavior, optional daemon `ToolList` and disabled-tool rebuild parity, Cairn validate/gates, formatting, and the smallest relevant Clankers Rust checks before sync/archive [r[steel-lisp-runtime.explicit-surfaces]] [r[steel-lisp-runtime.verification-contracts]]
