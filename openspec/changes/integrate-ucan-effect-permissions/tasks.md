## Phase 1: Spec Foundation

- [x] [serial] Create the UCAN effect-permission OpenSpec package. [covers=ucan-effect-permissions.*] [evidence=openspec/changes/integrate-ucan-effect-permissions]
- [x] [serial] Run proposal/design/tasks gates and resolve review findings. [covers=ucan-effect-permissions.*] [evidence=openspec/changes/integrate-ucan-effect-permissions/gate-report.md,openspec-validate] ✅ 10m (started: 2026-05-17T03:41:48Z → completed: 2026-05-17T03:51:46Z)

## Phase 2: Dependency and Adapter Seam

- [x] [serial] Add a Clankers UCAN authorization adapter that consumes only public `../ucan/` APIs. [covers=ucan-effect-permissions.ucan-adapter.public-api] [evidence=cargo-check-adapter,cargo-test-adapter] ✅ 6m (started: 2026-05-17T03:52:28Z → completed: 2026-05-17T03:58:42Z)
- [x] [serial] Record the UCAN source/pinning strategy and unsupported-sibling-checkout release behavior. [covers=ucan-effect-permissions.ucan-adapter.reproducible-source] [evidence=crates/clankers-ucan/UCAN_SOURCE.md] ✅ 1m (started: 2026-05-17T03:59:24Z → completed: 2026-05-17T04:00:05Z)
- [x] [serial] Define stable Clankers ability strings and URI normalization fixtures for file, shell, network, secret, browser, scheduler, remote, provider, delivery, artifact, plugin, and MCP effects. [covers=ucan-effect-permissions.effect-vocabulary.known-effect,ucan-effect-permissions.effect-vocabulary.unknown-effect] [evidence=vocabulary-fixture-tests] ✅ 3m (started: 2026-05-17T04:00:54Z → completed: 2026-05-17T04:03:10Z)
- [x] [serial] Define deterministic path, command, timeout, and max-bytes caveat hooks. [covers=ucan-effect-permissions.caveat-policy.path-command] [evidence=caveat-policy-tests] ✅ 2m (started: 2026-05-17T04:06:36Z → completed: 2026-05-17T04:08:26Z)
- [x] [parallel] Define deterministic network host, scheme, provider, and model-scope caveat hooks. [covers=ucan-effect-permissions.caveat-policy.network-provider] [evidence=caveat-policy-tests] ✅ 6m (started: 2026-05-17T04:08:59Z → completed: 2026-05-17T04:14:52Z)
- [x] [parallel] Define deterministic artifact hash, artifact kind, and redaction-class caveat hooks. [covers=ucan-effect-permissions.caveat-policy.artifact-redaction] [evidence=caveat-policy-tests] ✅ 1m (started: 2026-05-17T04:15:19Z → completed: 2026-05-17T04:16:22Z)
- [x] [parallel] Define deterministic expiry, not-before, nonce, and freshness-window caveat hooks. [covers=ucan-effect-permissions.caveat-policy.freshness,ucan-effect-permissions.caveat-policy.unknown-denies] [evidence=caveat-policy-tests] ✅ 2m (started: 2026-05-17T04:16:50Z → completed: 2026-05-17T04:18:33Z)

## Phase 3: Admission Integration

- [x] [serial] Route one low-risk built-in effect through UCAN admission before handler execution. [covers=ucan-effect-permissions.handler-admission.allow,ucan-effect-permissions.handler-admission.deny,effect-ability-runtime.handlers.ucan-denial] [evidence=runtime_admission-tests] ✅ completed: 2026-05-17T04:38:24Z
- [ ] [serial] Preserve existing human confirmation/admission ordering after UCAN allow decisions. [covers=ucan-effect-permissions.handler-admission.confirmation-order,effect-ability-runtime.handlers.confirmation-order] [evidence=confirmation-order-tests]
- [ ] [parallel] Add subagent/session delegation helpers that attenuate parent authority. [covers=ucan-effect-permissions.delegation.no-widening,ucan-effect-permissions.delegation.child-denied] [evidence=delegation-tests]
- [ ] [parallel] Add replay/revocation admission integration using caller-owned UCAN hooks. [covers=ucan-effect-permissions.replay-revocation.duplicate,ucan-effect-permissions.replay-revocation.revoked] [evidence=replay-revocation-tests]

## Phase 4: Receipts, Ledger, and Remote Sync

- [ ] [serial] Extend effect receipts and content-addressed artifact envelopes with redacted UCAN authorization metadata. [covers=ucan-effect-permissions.authorization-receipts.allowed,ucan-effect-permissions.authorization-receipts.denied-redacted,content-addressed-agent-artifacts.receipts.replay,content-addressed-agent-artifacts.receipts.redaction] [evidence=receipt-redaction-tests]
- [ ] [parallel] Persist safe typed ledger facts for authorization decisions and denials. [covers=ucan-effect-permissions.ledger-facts.query-denial,typed-durable-session-ledger.records.execution,typed-durable-session-ledger.records.redaction] [evidence=typed-ledger-authorization-tests]
- [ ] [parallel] Ensure remote/subagent artifact sync transmits only safe grant metadata/proof references and never secret token material. [covers=ucan-effect-permissions.remote-proof-sync.safe-reference,ucan-effect-permissions.remote-proof-sync.missing-authority,effect-ability-runtime.remote-deps.missing-safe,effect-ability-runtime.remote-deps.secret-denied] [evidence=remote-proof-sync-tests]

## Phase 5: Verification and Closeout

- [ ] [serial] Add positive and negative fixture coverage for vocabulary, admission allow/deny, caveats, revocation, replay, delegation attenuation, confirmation ordering, receipt redaction, ledger facts, and remote proof sync. [covers=ucan-effect-permissions.*] [evidence=cargo-nextest-focused]
- [ ] [serial] Run targeted Clankers checks plus the agreed `../ucan/` compatibility check. [covers=ucan-effect-permissions.ucan-adapter] [evidence=cargo-nextest-focused,ucan-compat-check,git-diff-check]
- [ ] [serial] Sync specs, archive the change, and commit with validation receipts. [covers=ucan-effect-permissions.*] [evidence=openspec-validate-all,archive-commit]
