# Tasks: Steel Self-Mutation Policy

## Planning and policy shape

- [ ] [serial] I1: Define CLI/session configuration for named Steel live-mutation runs, including default-deny behavior, visible mutation profile metadata, and separation from normal Steel eval/self-evolution candidate runs [r[steel-self-mutation-policy.explicit-opt-in]]
- [x] [serial] I2: Add Nickel-authored mutation policy contracts for target classes, path scopes, verbs, approval tiers, preflight gates, verification profiles, runtime profiles, redaction rules, and rollback requirements [r[steel-self-mutation-policy.nickel-policy]]
- [x] [parallel] I3: Add a policy export/check rail and checked fixture that rejects malformed classes, verbs, scopes, approvals, verification profiles, and rollback requirements [r[steel-self-mutation-policy.nickel-policy.export-contract]]

## UCAN and host enforcement

- [x] [serial] I4: Define stable UCAN ability/resource vocabulary for propose/apply/commit/rollback mutation operations and the safe receipt metadata shape [r[steel-self-mutation-policy.ucan-authority]]
- [x] [parallel] I5: Add authority validation that rejects missing, expired, revoked, wrong-audience, wrong-resource, wrong-verb, or over-delegated UCAN proofs before mutation [r[steel-self-mutation-policy.ucan-authority.denied]]
- [x] [serial] I6: Implement typed Steel host-function DTOs for proposing, applying, committing, and rolling back mutation requests without exposing raw filesystem/process/git/network/provider/credential authority [r[steel-self-mutation-policy.host-functions]]
- [ ] [serial] I7: Route host functions through Rust enforcement code that checks Nickel policy, UCAN authority, disabled-tool/session capability parity, approval state, and target normalization before writing; first pure enforcement core now covers exported policy, UCAN, approval state, target normalization, and byte-write patch gating before any host write [r[steel-self-mutation-policy.host-functions.apply-through-rust]]

## Mutation safety and receipts

- [ ] [serial] I8: Implement preflight for target classification, path normalization, path/symlink escape rejection, target hash capture, dirty-WIP policy, checkpoint/backup planning, policy hash capture, and approval state capture [r[steel-self-mutation-policy.receipts-and-preflight]]
- [ ] [parallel] I9: Emit deterministic redacted mutation receipts for allowed, denied, failed-verification, and rollback outcomes with Nickel policy hash and safe UCAN metadata only [r[steel-self-mutation-policy.receipts-and-preflight.safe-receipt]]
- [ ] [serial] I10: Run policy-selected verification after writes and block success/commit/promotion when verification fails [r[steel-self-mutation-policy.verification-and-rollback]]
- [ ] [serial] I11: Implement rollback guarded by recorded post-apply target hash and backup hash so operator edits are not clobbered [r[steel-self-mutation-policy.verification-and-rollback.guarded-rollback]]

## Verification and documentation

- [x] [parallel] V1: Add deterministic positive fixtures for a bounded skill or prompt mutation where Nickel policy and UCAN authorization match and verification passes; current fixture authorizes a bounded prompt apply request through the Rust preflight decision core [r[steel-self-mutation-policy.verification-fixtures.positive]]
- [ ] [parallel] V2: Add deterministic negative fixtures for path escape, missing/expired/wrong-resource UCAN, unauthorized verb, raw ambient write, failed verification, and stale rollback target; current policy/core fixtures cover path escape, unauthorized verb, ambient authority, missing/expired/revoked/wrong-audience/wrong-resource/wrong-verb/over-delegated UCAN, wildcard resources, missing approval, missing patch, missing UCAN requirement, and missing receipt policy hash [r[steel-self-mutation-policy.verification-fixtures.negative]]
- [x] [parallel] V3: Add architecture checks proving runtime enforcement consumes exported policy data or generated fixtures, while generic SDK/engine crates do not perform live Nickel evaluation for per-call mutation authority [r[steel-self-mutation-policy.nickel-policy.runtime-boundary]]
- [ ] [parallel] V4: Add Steel runtime tests proving mutation profiles still deny raw filesystem, shell, git, network, provider, credential, daemon, TUI, and native-tool access outside typed host functions [r[steel-self-mutation-policy.host-functions.raw-write-denied]]
- [ ] [parallel] V5: Add receipt redaction tests proving compact UCAN tokens, raw proofs, credentials, provider payloads, oversized patch bodies, and uncontrolled absolute-path dumps are never emitted [r[steel-self-mutation-policy.receipts-and-preflight.safe-receipt]]
- [ ] [serial] D1: Document the operator workflow, policy review checklist, UCAN grant shape, approval tiers, verification expectations, receipt review, and rollback process [r[steel-self-mutation-policy.explicit-opt-in.named-run]]

## Final gates

- [ ] [serial] G1: Run `nix run .#cairn -- validate --root .` [r[steel-self-mutation-policy.verification-fixtures]]
- [ ] [serial] G2: Run `nix run .#cairn -- gate proposal steel-self-mutation-policy --root .` [r[steel-self-mutation-policy.verification-fixtures]]
- [ ] [serial] G3: Run `nix run .#cairn -- gate design steel-self-mutation-policy --root .` [r[steel-self-mutation-policy.verification-fixtures]]
- [ ] [serial] G4: Run `nix run .#cairn -- gate tasks steel-self-mutation-policy --root .` [r[steel-self-mutation-policy.verification-fixtures]]
- [ ] [serial] G5: Run `git diff --check` [r[steel-self-mutation-policy.verification-fixtures]]
