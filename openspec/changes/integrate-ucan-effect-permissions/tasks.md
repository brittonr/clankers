## Phase 1: Spec Foundation

- [x] [serial] Create the UCAN effect-permission OpenSpec package. [covers=ucan-effect-permissions.*] [evidence=openspec/changes/integrate-ucan-effect-permissions]
- [ ] [serial] Run proposal/design/tasks gates and resolve review findings. [covers=ucan-effect-permissions.*] [evidence=openspec gate outputs]

## Phase 2: Dependency and Adapter Seam

- [ ] [serial] Add a Clankers UCAN authorization adapter that consumes only public `../ucan/` APIs and records the source/pinning strategy. [covers=ucan-effect-permissions.ucan-adapter]
- [ ] [parallel] Define stable Clankers effect ability strings and resource URI normalization fixtures. [covers=ucan-effect-permissions.effect-vocabulary]
- [ ] [parallel] Define deterministic Clankers caveat policy hooks for paths, commands, network hosts, artifact hashes, time, replay, and redaction classes. [covers=ucan-effect-permissions.caveat-policy]

## Phase 3: Admission Integration

- [ ] [serial] Route one low-risk built-in effect through UCAN admission before handler execution. [covers=ucan-effect-permissions.handler-admission]
- [ ] [parallel] Add subagent/session delegation helpers that attenuate parent authority. [covers=ucan-effect-permissions.delegation]
- [ ] [parallel] Add replay/revocation admission integration using caller-owned UCAN hooks. [covers=ucan-effect-permissions.replay-revocation]

## Phase 4: Receipts, Ledger, and Remote Sync

- [ ] [serial] Extend effect receipts and content-addressed artifact envelopes with redacted UCAN authorization metadata. [covers=ucan-effect-permissions.authorization-receipts]
- [ ] [parallel] Persist safe typed ledger facts for authorization decisions and denials. [covers=ucan-effect-permissions.ledger-facts]
- [ ] [parallel] Ensure remote/subagent artifact sync transmits only safe grant metadata/proof references and never secret token material. [covers=ucan-effect-permissions.remote-proof-sync]

## Phase 5: Verification and Closeout

- [ ] [serial] Add positive and negative fixture tests for allow, denial, caveat, revocation, replay, delegation attenuation, and receipt redaction. [covers=ucan-effect-permissions.*]
- [ ] [serial] Run targeted Clankers checks plus the agreed `../ucan/` compatibility check. [covers=ucan-effect-permissions.ucan-adapter]
- [ ] [serial] Sync specs, archive the change, and commit with validation receipts. [covers=ucan-effect-permissions.*]
