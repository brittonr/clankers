# Tasks

- [x] [serial] r[bind-harness-receipts-to-payload-commit.receipt-payload] Add top-level harness receipt payload metadata captured at run start.
- [x] [serial] r[bind-harness-receipts-to-payload-commit.index-verification] Verify selected receipt payload commits against indexed HEAD.
- [x] [parallel] r[bind-harness-receipts-to-payload-commit.docs] Document payload metadata and legacy receipt transition semantics.
- [x] [parallel] r[bind-harness-receipts-to-payload-commit.receipt-payload.emitted] Add/refresh harness contract coverage for the payload fields.
- [x] [parallel] r[bind-harness-receipts-to-payload-commit.index-verification.matching] Add evidence-index tests proving matching clean payload receipts are verified.
- [x] [parallel] r[bind-harness-receipts-to-payload-commit.index-verification.mismatch] Add evidence-index tests proving legacy, dirty, or mismatched payload receipts are not verified.
- [x] [serial] r[bind-harness-receipts-to-payload-commit.index-verification] Run focused tests, then rerun full harness and evidence-index after landing the clean payload commit.
