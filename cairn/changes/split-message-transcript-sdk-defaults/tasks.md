## Phase 1: Implementation

- [ ] [serial] I1: Inventory every production and test import of `clanker_message::{AgentMessage, MessageId, generate_id}` and `clanker_message::message::*`, then classify each as stable SDK or transcript compatibility. r[sdk-message-contract-boundary.default-subset] [covers=sdk-message-contract-boundary.default-subset]
- [ ] [serial] I2: Move transcript compatibility root exports behind an explicit module or non-default compatibility feature while preserving stable content/usage/streaming/semantic-event defaults. r[sdk-message-contract-boundary.transcript-compat-feature] [covers=sdk-message-contract-boundary.transcript-compat-feature]
- [ ] [serial] I3: Update desktop/session/provider/controller adapters to use the explicit transcript compatibility path instead of default SDK imports. r[sdk-message-contract-boundary.transcript-compat-feature] [covers=sdk-message-contract-boundary.transcript-compat-feature]
- [ ] [serial] I4: Strengthen message and embedded dependency rails so minimal SDK examples reject transcript internals plus `chrono`, `rand`, and `hex` unless the compatibility feature is explicitly enabled. r[sdk-message-contract-boundary.default-subset] [covers=sdk-message-contract-boundary.default-subset]

## Phase 2: Verification

- [ ] [serial] V1: Run message compatibility serialization fixtures and prove persisted Clankers transcript records still deserialize through the explicit compatibility path. r[sdk-message-contract-boundary.transcript-compat-feature] [covers=sdk-message-contract-boundary.transcript-compat-feature]
- [ ] [serial] V2: Run `scripts/check-message-contract-boundary.rs`, `scripts/check-embedded-sdk-deps.rs`, and `scripts/check-embedded-agent-sdk.rs`. r[sdk-message-contract-boundary.default-subset] [covers=sdk-message-contract-boundary.default-subset]
- [ ] [serial] V3: Run Cairn validation/gates for this change and `git diff --check`. r[sdk-message-contract-boundary.default-subset] [covers=sdk-message-contract-boundary.default-subset]
