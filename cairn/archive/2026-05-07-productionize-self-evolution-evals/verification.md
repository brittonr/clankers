# Verification Notes

## Integration Inventory

- Self-evolution CLI: `src/cli.rs` exposes `self-evolution run/approve/apply/rollback`; production profiles now extend `run` with `--profile` and `--corpus-manifest`.
- Self-evolution core: `src/self_evolution.rs` owns run receipts, approval/application/rollback guards, corpus manifest parsing, readiness labels, fake MCP/session-control receipts, and local deterministic scoring.
- Self-evolution command adapter: `src/commands/self_evolution.rs` loads candidate files, passes profile/corpus options into the core, and prints JSON/text receipts.
- Daemon/session-control seam: the current controlled-dogfood proof remains the existing fake MCP/session-control executor, which records safe `send_prompt`, `session_history`, and approval-confirmation receipts instead of hidden in-process mutation.
- Batch/eval seam: productionized readiness is local-manifest driven for this slice; remote datasets and hidden orchestration are intentionally not promotion evidence.

## Verification Commands

### Focused self-evolution unit/integration rail

- command: `cargo fmt --check && CARGO_TARGET_DIR=target cargo test --lib self_evolution -- --nocapture && CARGO_TARGET_DIR=target cargo check --tests`
- result: pass
- scope rationale: covers corpus parsing, invalid manifest rejection, readiness labels, unchanged-candidate control, failed evals, approval gating, application/rollback guards, CLI parser tests, and compile coverage for all tests.
- artifact: terminal transcript in session; focused test count `24 passed, 0 failed`.

### Controlled-dogfood CLI smoke

- command: `cargo run --quiet --bin clankers -- self-evolution run --target <tmp>/target.txt --baseline-command 'cargo test self_evolution' --candidate-output <tmp>/out --session sess-smoke --candidate-file <tmp>/candidate.txt --profile promotion-eligible --corpus-manifest <tmp>/corpus.json --dry-run --json`
- result: pass
- scope rationale: executes the real CLI against disposable files, verifies `readiness.label=promotion_eligible`, verifies `promotion_status=awaiting_human_approval`, verifies the isolated candidate exists, and verifies active target bytes remain unchanged.
- artifact: generated run id `self-evolution-a2b7cda0-cbce-486d-9365-720b94de7ff2` in `target/selfeval-smoke.*` temp output.

### OpenSpec and whitespace rail

- command: `openspec validate productionize-self-evolution-evals --strict && git diff --check`
- result: pass
- scope rationale: validates the active change package and whitespace after implementation/docs updates.

## Known Limitations

- The controlled-dogfood executor is still deterministic and local; it records the normal session-control receipt shape without launching a long live daemon evaluation.
- Corpus manifests are local JSON only; remote datasets are intentionally unsupported for promotion evidence in this slice.
- Readiness can be `promotion_eligible`, but active artifacts still require explicit human approval and explicit application/rollback guards.
