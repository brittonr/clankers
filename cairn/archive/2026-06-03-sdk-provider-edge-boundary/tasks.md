## Phase 1: Provider edge boundary

- [x] [serial] I1: Inventory provider/router/auth/discovery/request-shaping concerns and assign one owner per concern. r[sdk-provider-edge-boundary.concerns] [covers=sdk-provider-edge-boundary.concerns]
- [x] [serial] I2: Replace or isolate provider API use of display DTOs such as `clanker_tui_types::ThinkingLevel` behind neutral DTO conversion. r[sdk-provider-edge-boundary.neutral-model-api.no-display-dtos] [covers=sdk-provider-edge-boundary.neutral-model-api.no-display-dtos]
- [x] [parallel] I3: Keep generic SDK/provider-adapter examples on `ModelHost` or runtime neutral provider DTOs without `clankers-provider`/router/auth dependencies. r[sdk-provider-edge-boundary.neutral-model-api.sdk-host-owned] [covers=sdk-provider-edge-boundary.neutral-model-api.sdk-host-owned]
- [x] [parallel] I4: Collapse duplicate request/event projection code or add parity rails with explicit owner receipts. r[sdk-provider-edge-boundary.concerns.duplicate-abstractions] [covers=sdk-provider-edge-boundary.concerns.duplicate-abstractions]

## Phase 2: Verification

- [x] [serial] V1: Add literal request-shape/provider-adapter fixtures that do not call the body builder under test to construct expected JSON. r[sdk-provider-edge-boundary.verification.literal-fixtures] [covers=sdk-provider-edge-boundary.verification.literal-fixtures] [evidence=evidence/provider-fixtures.md]
- [x] [serial] V2: Add source/dependency rails rejecting provider/router/auth/TUI/protocol dependencies in generic SDK examples and green crates. r[sdk-provider-edge-boundary.verification.dependency-rails] [covers=sdk-provider-edge-boundary.verification.dependency-rails] [evidence=evidence/dependency-rails.md]
- [x] [serial] V3: Run provider fixture tests, embedded provider-adapter kit, SDK dependency checks, Cairn gates/validate, and relevant live-smoke documentation updates if contracts changed. r[sdk-provider-edge-boundary.verification] [covers=sdk-provider-edge-boundary.verification] [evidence=evidence/validation-closeout.md]
