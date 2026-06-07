# Tasks: Shrink Root Shell Dependency Budget

## Phase 1: Inventory

- [ ] [serial] R1: Classify every root internal dependency row as app-edge wiring, edge projection, adapter exception, or temporary policy with a convergence target. r[remaining-coupling-drain.root-shell-dependency-budget.inventory] [covers=remaining-coupling-drain.root-shell-dependency-budget.inventory]

## Phase 2: Implementation

- [ ] [serial] I1: Drain at least one temporary-policy root behavior slice into its owner crate or neutral adapter while preserving root parsing/assembly/projection only. r[remaining-coupling-drain.root-shell-dependency-budget.slice-drain] [covers=remaining-coupling-drain.root-shell-dependency-budget.slice-drain]
- [ ] [serial] I2: Update the root ownership receipt to lower the temporary-policy budget, lower the dependency budget, or narrow each remaining exception with a focused convergence condition. r[remaining-coupling-drain.root-shell-dependency-budget.budget-evidence] [covers=remaining-coupling-drain.root-shell-dependency-budget.budget-evidence]

## Phase 3: Verification

- [ ] [serial] V1: Run focused tests for the drained root slice plus CLI/TUI/daemon smoke where the slice affects user-visible behavior. r[remaining-coupling-drain.root-shell-dependency-budget.behavior-validation] [covers=remaining-coupling-drain.root-shell-dependency-budget.behavior-validation]
- [ ] [serial] V2: Run the root ownership rail, `cargo check --tests` for affected crates, Cairn gates/validate, and `git diff --check` before closeout. r[remaining-coupling-drain.root-shell-dependency-budget.closeout] [covers=remaining-coupling-drain.root-shell-dependency-budget.closeout]
