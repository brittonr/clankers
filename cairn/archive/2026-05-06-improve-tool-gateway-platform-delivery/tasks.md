## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `improve-tool-gateway-platform-delivery`.
- [x] Validate the OpenSpec package with `openspec validate improve-tool-gateway-platform-delivery --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory current `tool-gateway-platform-delivery` code/docs seams and record the exact files to touch. Evidence: `verification.md` inventory lists policy, CLI/tool, standalone/daemon rebuild, tests, and docs seams.
- [x] Add typed policy/config/request/receipt models with unit tests. Evidence: `src/tool_gateway.rs` adds `GatewayMode`, `GatewayToolPolicyReceipt`, `ArtifactKind`, `PlatformDeliveryReceipt`, shared filter helpers, and unit tests.
- [x] Implement the first runtime/adapter slice behind deterministic fake tests. Evidence: `local_delivery_receipt` plus `tool_gateway` `deliver_receipt` action and `tests/gateway.rs` cover safe local/session and unsupported remote receipts without live delivery.
- [x] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy. Evidence: standalone initial/rebuild and daemon create/rebuild paths call `crate::tool_gateway::allowed_tools_for_policy` with mode-specific active toolsets.
- [x] Update README and relevant docs for supported behavior, non-goals, and safety policy. Evidence: README, config reference, and request lifecycle docs updated.

## Phase 3: Verification and Closeout

- [x] Run targeted package/integration checks for the touched modules. Evidence: `cargo test --lib gateway` and `cargo test --test gateway` passed.
- [x] Run `cargo check --tests` for affected crates. Evidence: `CARGO_TARGET_DIR=target cargo check --tests` passed.
- [x] Run `git diff --check`. Evidence: passed after implementation.
- [x] Sync the delta spec into the canonical `tool-gateway-platform-delivery` spec and archive the change after implementation tasks complete. Evidence: `openspec archive improve-tool-gateway-platform-delivery --yes` archived the change and applied `+ 2` requirements to the canonical spec.
