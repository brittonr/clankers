## Phase 1: Neutral DTO import drain

- [x] [serial] I1: Inventory `clankers-agent` provider imports by module and classify each as neutral DTO reexport, provider-native model adapter, test-only fixture, or removable coupling. r[agent-provider-neutral-dtos.neutral-imports] [covers=agent-provider-neutral-dtos.neutral-imports]
- [x] [serial] I2: Replace `clankers_provider::message`, `clankers_provider::Usage`, and provider streaming reexport imports in reusable agent modules with `clanker-message` equivalents. r[agent-provider-neutral-dtos.neutral-imports.no-provider-reexports] [covers=agent-provider-neutral-dtos.neutral-imports.no-provider-reexports]
- [x] [serial] I3: Confine `CompletionRequest`, `Provider`, and provider-native stream execution references to named model adapter modules with explicit owner receipts. r[agent-provider-neutral-dtos.model-adapter.completion-request] [covers=agent-provider-neutral-dtos.model-adapter.completion-request]
- [x] [serial] I4: Introduce or document the neutral model request/stream DTO seam that will let `AgentModelPort` target runtime provider services instead of provider-native requests in the next slice. r[agent-provider-neutral-dtos.runtime-model-seam] [covers=agent-provider-neutral-dtos.runtime-model-seam]

## Phase 2: Verification

- [x] [serial] V1: Add or update source-boundary rails that reject provider reexport imports in reusable agent policy modules and name `clanker-message` or model adapter replacements. r[agent-provider-neutral-dtos.verification.import-rail] [covers=agent-provider-neutral-dtos.verification.import-rail] [evidence=evidence/validation.md]
- [x] [serial] V2: Run focused agent tests for turn transcript, execution, compaction, event conversion, and Steel tool/planning paths after the import migration. r[agent-provider-neutral-dtos.model-adapter.turn-policy] [covers=agent-provider-neutral-dtos.model-adapter.turn-policy] [evidence=evidence/validation.md]
- [x] [serial] V3: Run `cargo check -p clankers-agent --tests`, lego architecture boundary rail, Cairn gates/validate for this change, and `git diff --check`. r[agent-provider-neutral-dtos.verification.closeout] [covers=agent-provider-neutral-dtos.verification.closeout] [evidence=evidence/validation.md]
