# Final validation evidence

Evidence-ID: harden-embedded-sdk-api-inventory.final-validation
Artifact-Type: command-output-summary
Task-ID: V3
Covers: embedded-composition-kits.api-inventory-stability
Date: 2026-06-04
Status: PASS

## Commands

```text
git diff --check
nix run .#cairn -- validate --root .
nix run .#cairn -- gate proposal harden-embedded-sdk-api-inventory --root .
nix run .#cairn -- gate design harden-embedded-sdk-api-inventory --root .
nix run .#cairn -- gate tasks harden-embedded-sdk-api-inventory --root .
```

## Relevant output

```text
git diff --check
exit=0

nix run .#cairn -- validate --root .
{
  "change_issues": [],
  "changes": 5,
  "issues": [],
  "layout": "cairn",
  "policy": "cairn-default",
  "spec_issues": [],
  "specs_validated": 58,
  "valid": true
}
exit=0

nix run .#cairn -- gate proposal harden-embedded-sdk-api-inventory --root .
verdict=PASS valid=true receipt_hash=075e7bd59631231fa251a874be3172568eaa2ae1c9cb8cd370b92a7f5b4aa4ee
exit=0

nix run .#cairn -- gate design harden-embedded-sdk-api-inventory --root .
verdict=PASS valid=true receipt_hash=0ec753cd15cb46eee5d29fb2e3391a90ce44c02981da692a9ebc1b4d3863c9ac
exit=0

nix run .#cairn -- gate tasks harden-embedded-sdk-api-inventory --root .
verdict=PASS valid=true receipt_hash=012cc9a4aff26f564966ff9cc9215a8ff55f2bf7db73c36c1e0d4a94db6853e1
exit=0
```
