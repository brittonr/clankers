Task-ID: V1
Covers: sdk-session-ledger-boundary.verification.resume-fixtures
Artifact-Type: validation-evidence

# Session Resume Fixtures

## Fixture update

Updated `examples/embedded-session-store/session-resume-evidence.json` with `boundary_rail: scripts/check-session-ledger-boundary.rs` so the resume fixture packet explicitly points at the ledger-boundary owner rail.

## Validated scenarios

`scripts/check-session-resume-brick.rs` validates:

- `embedded-session-store` restores user/assistant context through product-owned session/message DTOs and fails closed for missing sessions.
- `embedded-product-workbench` restores user/tool/assistant context through product-owned session/message DTOs and fails closed for missing sessions.
- `clankers-runtime` resume tests restore ordered neutral `SessionLedgerEntry` context and fail before model/tool execution for missing or unsupported stores.

## Command

`nix develop -c cargo -q -Zscript scripts/check-session-resume-brick.rs`

Result: runtime session resume tests passed and `target/embedded-sdk-release/session-resume-brick-receipt.json` was written.
