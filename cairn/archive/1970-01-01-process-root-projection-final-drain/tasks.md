## Phase 1: Implementation

- [x] [serial] I1: Inventory every remaining native service type/function in `src/tools/process.rs` and classify as root projection or native service ownership. r[process-root-projection-final-drain.native-service-owner] [covers=process-root-projection-final-drain.native-service-owner]
- [x] [serial] I2: Move `ProcessEntry`, native status/receipt helpers, and `NativeProcessJobService` into the native owner module without changing public receipts. r[process-root-projection-final-drain.native-service-owner] [covers=process-root-projection-final-drain.native-service-owner]
- [x] [serial] I3: Move native service tests to the native owner and leave root tests focused on JSON parsing, backend selection, and envelope projection. r[process-root-projection-final-drain.root-projection] [covers=process-root-projection-final-drain.root-projection]
- [x] [serial] I4: Update process and lego architecture rails to reject native service policy in `src/tools/process.rs`. r[process-root-projection-final-drain.verification] [covers=process-root-projection-final-drain.verification]

## Phase 2: Verification

- [x] [serial] V1: Run focused native/process tests and prove user-facing process receipts are unchanged. r[process-root-projection-final-drain.verification] [covers=process-root-projection-final-drain.verification] [evidence=evidence/native-service-drain.md]
- [x] [serial] V2: Run `cargo check -p clankers --tests`, process boundary rails, lego architecture rails, Cairn gates/validate, and `git diff --check`. r[process-root-projection-final-drain.verification] [covers=process-root-projection-final-drain.verification] [evidence=evidence/closeout-validation.md]
