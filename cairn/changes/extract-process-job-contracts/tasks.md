## Phase 1: Implementation

- [x] [serial] I1: Inventory current process-job DTOs/policy and choose the green contract owner crate or module. r[remaining-coupling-drain.process-job-policy.neutral-contract-owner] [covers=remaining-coupling-drain.process-job-policy.neutral-contract-owner] [evidence=evidence/process-job-contracts.md]
- [x] [serial] I2: Move native admission DTOs and the pure admission decision helper behind the chosen green owner while preserving runtime compatibility reexports. r[remaining-coupling-drain.process-job-policy.neutral-contract-owner] [covers=remaining-coupling-drain.process-job-policy.neutral-contract-owner] [evidence=evidence/process-job-contracts.md]
- [x] [serial] I3: Move safe profile receipt metadata constants/DTO behind the chosen green owner while preserving runtime compatibility reexports. r[remaining-coupling-drain.process-job-policy.neutral-contract-owner] [covers=remaining-coupling-drain.process-job-policy.neutral-contract-owner] [evidence=evidence/process-job-contracts.md]
- [ ] [serial] I4: Move remaining receipt/redaction/retention/notification DTOs behind the chosen owner and leave root/backend code as adapters. r[remaining-coupling-drain.process-job-policy.neutral-contract-owner] [covers=remaining-coupling-drain.process-job-policy.neutral-contract-owner] [evidence=evidence/process-job-contracts.md]
- [x] [serial] I5: Refresh runtime facade and embedded SDK inventories for migrated admission/profile receipt contracts. r[remaining-coupling-drain.process-job-policy.neutral-contract-owner] [covers=remaining-coupling-drain.process-job-policy.neutral-contract-owner] [evidence=evidence/process-job-contracts.md]

## Phase 2: Verification

- [ ] [serial] V1: Run focused process-job policy/backend tests and runtime/process architecture rails. r[remaining-coupling-drain.process-job-policy.neutral-contract-owner] [covers=remaining-coupling-drain.process-job-policy.neutral-contract-owner] [evidence=evidence/process-job-contracts.md]
- [ ] [serial] V2: Run Cairn validation/gates, `git diff --check`, and the appropriate aggregate SDK/FCIS acceptance rail. r[remaining-coupling-drain.process-job-policy.neutral-contract-owner] [covers=remaining-coupling-drain.process-job-policy.neutral-contract-owner] [evidence=evidence/validation-closeout.md]
