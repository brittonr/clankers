## Phase 1: Neutral service context

- [ ] [serial] I1: Inventory concrete services currently threaded through `ControllerToolPort` and legacy tool execution, including DB, search index, hook pipeline, event/progress sender, capability gate, cancellation, and Steel tool substrate policy. r[neutral-tool-service-context.service-contracts] [covers=neutral-tool-service-context.service-contracts]
- [ ] [serial] I2: Define neutral tool service traits/DTOs for storage/search, hook decisions, progress/events, capability checks, cancellation, and optional runtime policy. r[neutral-tool-service-context.service-contracts.no-shell-fields] [covers=neutral-tool-service-context.service-contracts.no-shell-fields]
- [ ] [serial] I3: Update `ControllerToolPort` and the legacy tool adapter to pass a neutral service bundle instead of concrete DB/hook/event fields through reusable tool execution. r[neutral-tool-service-context.controller-tool-port.edge-owned] [covers=neutral-tool-service-context.controller-tool-port.edge-owned]
- [ ] [parallel] I4: Migrate one storage/search tool path and one hook/progress-emitting tool path to consume the neutral services. r[neutral-tool-service-context.representative-migration] [covers=neutral-tool-service-context.representative-migration]

## Phase 2: Verification

- [ ] [serial] V1: Add neutral service fixtures for success, missing service, hook continue/modify/deny, capability denial, cancellation, progress emission, and legacy adapter parity. r[neutral-tool-service-context.verification.fixtures] [covers=neutral-tool-service-context.verification.fixtures]
- [ ] [serial] V2: Add or update source-boundary rails rejecting concrete DB/hook/TUI/protocol/root imports in reusable tool-host context modules. r[neutral-tool-service-context.verification.boundary-rail] [covers=neutral-tool-service-context.verification.boundary-rail]
- [ ] [serial] V3: Run focused tool-host/agent tests, `cargo check -p clankers-agent -p clankers-tool-host --tests`, lego architecture rail, Cairn gates/validate, and `git diff --check`. r[neutral-tool-service-context.verification] [covers=neutral-tool-service-context.verification]
