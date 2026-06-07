## Phase 1: Implementation

- [x] [serial] I1: Inventory root `src/` modules and dependency edges into shell-wiring, edge-projection, adapter-exception, and temporary-policy buckets. r[remaining-coupling-drain.root-shell-policy.root-module-ownership-map] [covers=remaining-coupling-drain.root-shell-policy.root-module-ownership-map] [evidence=evidence/root-shell-policy.md]
- [x] [serial] I2: Select one temporary-policy root cluster and move reusable behavior to its named workspace owner or document the adapter exception. r[remaining-coupling-drain.root-shell-policy.policy-slice-drain] [covers=remaining-coupling-drain.root-shell-policy.policy-slice-drain] [evidence=evidence/root-shell-policy.md]
- [x] [serial] I3: Refresh architecture docs, owner receipts, and `policy/lego-architecture/dependency-ownership-baseline.json` with the smaller root convergence condition. r[remaining-coupling-drain.root-shell-policy.root-module-ownership-map] [covers=remaining-coupling-drain.root-shell-policy.root-module-ownership-map] [evidence=evidence/root-shell-policy.md]

## Phase 2: Verification

- [x] [serial] V1: Run focused tests for the moved owner plus `scripts/check-lego-architecture-boundaries.rs` and `cargo nextest run -p clankers-controller --test fcis_shell_boundaries` when the touched seam is covered by FCIS. r[remaining-coupling-drain.root-shell-policy.policy-slice-drain] [covers=remaining-coupling-drain.root-shell-policy.policy-slice-drain] [evidence=evidence/root-shell-policy.md]
- [x] [serial] V2: Run Cairn validation/gates, `git diff --check`, and any user-visible daemon/attach/runtime parity rails affected by the root slice. r[remaining-coupling-drain.root-shell-policy.root-module-ownership-map] [covers=remaining-coupling-drain.root-shell-policy.root-module-ownership-map] [evidence=evidence/validation-closeout.md]
