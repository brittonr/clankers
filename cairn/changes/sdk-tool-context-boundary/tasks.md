## Phase 1: Tool context drain

- [ ] [serial] I1: Inventory built-in and plugin tools that consume `ToolContext` DB, search, hooks, event/progress, session, or cancellation services. r[sdk-tool-context-boundary.inventory] [covers=sdk-tool-context-boundary.inventory]
- [ ] [serial] I2: Mark `ToolContext` as compatibility-only in docs/rails and identify neutral service replacements for each service family. r[sdk-tool-context-boundary.legacy-context.compatibility-only] [covers=sdk-tool-context-boundary.legacy-context.compatibility-only]
- [ ] [parallel] I3: Migrate at least one storage/search tool path and one hook/progress path to neutral `ToolHostServices`. r[sdk-tool-context-boundary.neutral-services.representative-tools] [covers=sdk-tool-context-boundary.neutral-services.representative-tools]
- [ ] [parallel] I4: Add fail-closed behavior for migrated tools when required neutral services are absent. r[sdk-tool-context-boundary.neutral-services.missing-service] [covers=sdk-tool-context-boundary.neutral-services.missing-service]

## Phase 2: Verification

- [ ] [serial] V1: Add tests for migrated neutral tools covering success, missing service, progress emission, hook/capability denial, and cancellation. r[sdk-tool-context-boundary.verification.fixtures] [covers=sdk-tool-context-boundary.verification.fixtures]
- [ ] [serial] V2: Add or update rails that reject new direct DB/hook/TUI/protocol/root service imports in reusable tool-host code and migrated neutral tools. r[sdk-tool-context-boundary.verification.boundary-rail] [covers=sdk-tool-context-boundary.verification.boundary-rail]
- [ ] [serial] V3: Run focused tool-host/agent/root tool tests, lego architecture rail, Cairn gates/validate, and `git diff --check`. r[sdk-tool-context-boundary.verification] [covers=sdk-tool-context-boundary.verification]
