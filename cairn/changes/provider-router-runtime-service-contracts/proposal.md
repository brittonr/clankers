# Proposal: Provider Router Runtime Service Contracts

## Problem

The embedded runtime has provider/router/auth service names, but the contract is too lossy for real Clankers model execution. `ProviderExecutionRequest` carries a prompt string and returns an `ExtensionReceipt`/stream stats, not a neutral model stream or response. Desktop adapters still build provider-native `CompletionRequest` values in root-edge code, and auth refresh/login/credential-pool behavior remains desktop-specific.

## Proposed Change

Define a real runtime provider service contract: neutral model request/stream/response DTOs, safe auth-store operations, credential-pool selection, retry/refresh receipts, and explicit desktop adapters that delegate to `clankers-provider`/`clanker-router` without leaking provider-native shapes into the SDK boundary.

## Impact

- **Files**: `crates/clankers-runtime/src/services.rs`, `src/runtime_services.rs`, `crates/clankers-provider/src/router_request_bridge.rs`, provider fixtures, docs/API inventory.
- **Testing**: literal request/response fixtures, redaction checks, desktop adapter parity for routing/auth/provider failure paths.
