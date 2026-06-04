# API inventory rail evidence

Evidence-ID: harden-embedded-sdk-api-inventory.api-inventory-rails
Artifact-Type: command-output-summary
Task-ID: V1
Covers: embedded-composition-kits.api-inventory-typed, embedded-composition-kits.api-inventory-typed.methods-fields-reexports, embedded-composition-kits.api-inventory-typed.test-only-exclusion, embedded-composition-kits.api-inventory-stability, embedded-composition-kits.api-inventory-stability.stable-hash, embedded-composition-kits.api-inventory-stability.owner-diagnostics
Date: 2026-06-04
Status: PASS

## Commands

```text
scripts/check-embedded-sdk-api.rs
scripts/check-brick-inventory-stability.rs
```

## Relevant output

```text
scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 620 public items (625 rows)
exit=0

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json
exit=0
```

## Inventory expansion summary

- `docs/src/generated/embedded-sdk-api.md` now records 625 rows over 620 scanned public Rust items plus 5 checked examples.
- New inventory kinds include `field`, `method`, and `reexport` rows.
- `policy/embedded-lego/brick-inventory-stability.json` now pins counts: total=625, supported=303, optional-support=67, compatibility-alias=0, experimental=187, unsupported-internal=68, stable-contract=370.
- Stable-contract hash is `bbf41d01f78f5a782ddd0dcda237b7b77a62975c462db7880364b35aa2046e04`.
- The typed scanner self-test covers a feature-gated public type, public field, public method, trait method, root reexport, test-only exclusion, and a runtime public item after `#[cfg(test)]`.
- Missing/wrong-owner diagnostics include source path and remediation guidance to add an inventory row, hide the item, move it to an app-edge module, or update migration notes/policy.
