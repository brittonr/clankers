## Phase 1: Implementation

- [x] [serial] I1: Inventory every production and test import of `clanker_message::{AgentMessage, MessageId, generate_id}` and `clanker_message::message::*`, then classify each as stable SDK or transcript compatibility. r[sdk-message-contract-boundary.default-subset] [covers=sdk-message-contract-boundary.default-subset] [evidence=evidence/implementation-boundary.md]
- [x] [serial] I2: Move transcript compatibility root exports behind an explicit module or non-default compatibility feature while preserving stable content/usage/streaming/semantic-event defaults. r[sdk-message-contract-boundary.transcript-compat-feature] [covers=sdk-message-contract-boundary.transcript-compat-feature] [evidence=evidence/implementation-boundary.md]
- [x] [serial] I3: Update desktop/session/provider/controller adapters to use the explicit transcript compatibility path instead of default SDK imports. r[sdk-message-contract-boundary.transcript-compat-feature] [covers=sdk-message-contract-boundary.transcript-compat-feature] [evidence=evidence/implementation-boundary.md]
- [x] [serial] I4: Strengthen message and embedded dependency rails so minimal SDK examples reject transcript internals plus `chrono`, `rand`, and `hex` unless the compatibility feature is explicitly enabled. r[sdk-message-contract-boundary.default-subset] [covers=sdk-message-contract-boundary.default-subset] [evidence=evidence/implementation-boundary.md]

## Phase 2: Verification

- [x] [serial] V1: Run message compatibility serialization fixtures and prove persisted Clankers transcript records still deserialize through the explicit compatibility path. r[sdk-message-contract-boundary.transcript-compat-feature] [covers=sdk-message-contract-boundary.transcript-compat-feature] [evidence=evidence/transcript-compat-fixtures.md]
- [x] [serial] V2: Run `scripts/check-message-contract-boundary.rs`, `scripts/check-embedded-sdk-deps.rs`, and `scripts/check-embedded-agent-sdk.rs`. r[sdk-message-contract-boundary.default-subset] [covers=sdk-message-contract-boundary.default-subset] [evidence=evidence/validation-closeout.md]
- [x] [serial] V3: Run Cairn validation/gates for this change and `git diff --check`. r[sdk-message-contract-boundary.default-subset] [covers=sdk-message-contract-boundary.default-subset] [evidence=evidence/validation-closeout.md]
