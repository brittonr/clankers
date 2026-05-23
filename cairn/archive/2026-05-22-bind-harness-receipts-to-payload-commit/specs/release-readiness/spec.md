# release-readiness Specification Delta

## ADDED Requirements

### Requirement: Harness receipts bind to payload commit [r[bind-harness-receipts-to-payload-commit.receipt-payload]]
Harness result receipts MUST record deterministic Git payload metadata captured once at harness start so downstream evidence can distinguish current-HEAD validation from historical local receipts.

#### Scenario: Harness result includes payload metadata [r[bind-harness-receipts-to-payload-commit.receipt-payload.emitted]]
- GIVEN a developer runs any `./scripts/test-harness.sh` mode
- WHEN the harness writes `results.json`
- THEN the receipt includes a top-level `payload.commit` equal to the HEAD being tested
- AND it records payload branch, describe string, tracked dirty state, upstream, and ahead/behind status when available

### Requirement: Evidence index verifies receipt payload commits [r[bind-harness-receipts-to-payload-commit.index-verification]]
The current-head evidence index MUST mark selected receipts as payload-commit verified only when their recorded payload commit matches the indexed HEAD and the receipt payload was captured from a clean tracked worktree.

#### Scenario: Matching clean payload receipt is current-head proof [r[bind-harness-receipts-to-payload-commit.index-verification.matching]]
- GIVEN a passed receipt records `payload.commit` equal to the current index HEAD
- AND the receipt records `payload.tracked_dirty=false`
- WHEN the evidence-index helper selects that receipt
- THEN it reports `payload_commit_verified=true`

#### Scenario: Legacy, dirty, or mismatched payload receipts are not overclaimed [r[bind-harness-receipts-to-payload-commit.index-verification.mismatch]]
- GIVEN a selected receipt has no payload metadata, a different payload commit, or `payload.tracked_dirty=true`
- WHEN the evidence-index helper writes the index
- THEN it reports `payload_commit_verified=false`
- AND it does not describe that receipt as current-HEAD validation

### Requirement: Payload binding documentation [r[bind-harness-receipts-to-payload-commit.docs]]
Release-readiness documentation MUST explain the payload metadata fields and the transition behavior for older receipts that lack payload metadata.

#### Scenario: Documentation describes legacy receipt semantics [r[bind-harness-receipts-to-payload-commit.docs.legacy]]
- GIVEN an operator reads the release-readiness reference
- WHEN it describes the current-head evidence index
- THEN it states that receipts lacking payload metadata may be selected as historical local evidence but cannot be marked payload-commit verified
