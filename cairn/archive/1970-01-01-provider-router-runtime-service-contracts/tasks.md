## Phase 1: Provider service contract

- [x] [serial] I1: Replace the prompt-string `ProviderExecutionRequest` with neutral model request/response/stream DTOs sufficient for engine-host execution. [covers=r[provider-router-runtime-services.model-contract.neutral-request]]
- [x] [serial] I2: Define provider service outcomes for completed response, streamed deltas, retryable failure, terminal failure, cancellation, and usage accounting. [covers=r[provider-router-runtime-services.model-contract.streamed-outcome]]
- [x] [parallel] I3: Split auth-store access, credential-pool selection, refresh persistence, pending login verifier storage, and provider routing into explicit host services with safe receipts. [covers=r[provider-router-runtime-services.auth-contract.host-owned]]
- [x] [serial] I4: Implement desktop adapters that delegate to `clankers-provider`/`clanker-router` owners without duplicating provider-native request shaping or router policy. [covers=r[provider-router-runtime-services.desktop-adapter.delegates-policy]]

## Phase 2: Verification

- [x] [parallel] V1: Add literal fixture tests for neutral request conversion, streamed response conversion, retryable/terminal failures, usage accounting, and redacted auth/provider receipts. [covers=r[provider-router-runtime-services.verification.literal-fixtures]]
- [x] [serial] V2: Add desktop adapter parity tests for known-provider fail-closed behavior, OpenAI Codex separation, credential-pool selection, and disabled embedded defaults. [covers=r[provider-router-runtime-services.verification.desktop-parity]]
- [x] [serial] V3: Run provider/runtime focused tests, embedded SDK rail if touched, Cairn validate/gates, and `git diff --check`. [covers=r[provider-router-runtime-services.verification.closeout]]
