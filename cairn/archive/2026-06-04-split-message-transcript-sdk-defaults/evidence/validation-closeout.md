# Validation closeout evidence

Evidence-ID: split-message-transcript-sdk-defaults.validation-closeout
Artifact-Type: command-output-summary
Task-ID: V2,V3
Covers: sdk-message-contract-boundary.default-subset, sdk-message-contract-boundary.default-subset.minimal-graph, sdk-message-contract-boundary.default-subset.root-exports, sdk-message-contract-boundary.transcript-compat-feature, sdk-message-contract-boundary.transcript-compat-feature.opt-in, sdk-message-contract-boundary.transcript-compat-feature.serialization
Date: 2026-06-04
Status: PASS

## Commands completed before Cairn closeout

```text
scripts/check-message-contract-boundary.rs
scripts/check-embedded-sdk-deps.rs
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
scripts/check-embedded-sdk-api.rs
scripts/check-brick-inventory-stability.rs
scripts/check-session-ledger-boundary.rs
```

## Relevant output

```text
scripts/check-message-contract-boundary.rs
ok: message contract boundary rail passed
exit=0

scripts/check-embedded-sdk-deps.rs
ok: embedded SDK example dependency graph has 56 packages and excludes forbidden runtime crates
exit=0

pueue task 10: split-message-transcript-acceptance-rerun
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
Start: Thu, 4 Jun 2026 01:15:50 -0400
End:   Thu, 4 Jun 2026 01:23:16 -0400
embedded-agent-sdk acceptance passed
exit=0

scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 610 public items (615 rows)
exit=0

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json
exit=0

scripts/check-session-ledger-boundary.rs
ok: session ledger boundary inventory covers 15 paths
exit=0
```

## Cairn and diff hygiene

```text
git diff --check
exit=0

nix run .#cairn -- validate --root .
{
  "change_issues": [],
  "changes": 4,
  "issues": [],
  "layout": "cairn",
  "policy": "cairn-default",
  "spec_issues": [],
  "specs_validated": 57,
  "valid": true
}
exit=0

nix run .#cairn -- gate proposal split-message-transcript-sdk-defaults --root .
verdict=PASS
receipt_hash=afa4d45c3330e021083a24321e3da9ac86d741cc1f40b6f39b1cf44195d71b74
exit=0

nix run .#cairn -- gate design split-message-transcript-sdk-defaults --root .
verdict=PASS
receipt_hash=58ad7534884eebfd4216641c6d54997bccd75db56738e770a5027f595559a805
exit=0

nix run .#cairn -- gate tasks split-message-transcript-sdk-defaults --root .
verdict=PASS
receipt_hash=e82689c9d9064f63e9c02d1901cf4dd2df797f3cf526beffe6302eba825e70dd
exit=0
```

Final post-evidence rerun:

```text
git diff --check
exit=0

nix run .#cairn -- validate --root .
valid=true
exit=0

nix run .#cairn -- gate tasks split-message-transcript-sdk-defaults --root .
verdict=PASS
receipt_hash=5865613e04a16bf348d1c05c6ee5e9796f0a20abe7e54be3a72df163d86fd7fe
exit=0
```
