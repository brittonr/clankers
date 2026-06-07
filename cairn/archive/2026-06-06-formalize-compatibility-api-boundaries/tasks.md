## Phase 1: Implementation

- [x] [serial] I1: Inventory current optional-support, compatibility-alias, and unsupported-internal APIs that are really compatibility surfaces. r[sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [covers=sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [evidence=evidence/compatibility-api-boundaries.md]
- [x] [serial] I2: Add owner/fixture metadata or docs for each compatibility surface touched in this slice. r[sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [covers=sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [evidence=evidence/compatibility-api-boundaries.md]
- [x] [serial] I3: Extend inventory/boundary rails so default SDK exports and examples cannot consume compatibility APIs without explicit opt-in. r[sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [covers=sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [evidence=evidence/compatibility-api-boundaries.md]

## Phase 2: Verification

- [x] [serial] V1: Run message contract boundary, SDK inventory/budget rails, and any provider/router parity rails touched by compatibility metadata. r[sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [covers=sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [evidence=evidence/compatibility-api-boundaries.md]
- [x] [serial] V2: Run Cairn validation/gates, `git diff --check`, and aggregate embedded SDK acceptance if inventory labels move. r[sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [covers=sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures] [evidence=evidence/validation-closeout.md]
