# Tasks

- [ ] [serial] T1. Define the canonical command and required receipt-field list for docs validation. [covers=r[bg-process-tui-dogfood-doc-checker.command-drift],r[bg-process-tui-dogfood-doc-checker.receipt-criteria]]
- [ ] [parallel] T2. Implement the focused docs checker or tests with positive and negative coverage. [covers=r[bg-process-tui-dogfood-doc-checker.negative-fixture],r[bg-process-tui-dogfood-doc-checker.runtime-boundary]]
- [ ] [parallel] T3. Update docs only where needed to satisfy the checker. [covers=r[bg-process-tui-dogfood-doc-checker.command-drift],r[bg-process-tui-dogfood-doc-checker.receipt-criteria]]
- [ ] [serial] T4. Verify checker/tests, `./scripts/test-harness.sh dogfood bg-process-tui` when runtime proof is needed, and `git diff --check`. [covers=r[bg-process-tui-dogfood-doc-checker.negative-fixture],r[bg-process-tui-dogfood-doc-checker.runtime-boundary]]
