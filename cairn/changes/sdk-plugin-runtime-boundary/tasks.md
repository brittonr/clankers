## Phase 1: Plugin boundary split

- [ ] [serial] I1: Inventory plugin responsibilities as manifest schema, runtime dispatch, sandbox/launch policy, tool registration, supervision, hooks, host events, and UI projection. r[sdk-plugin-runtime-boundary.inventory] [covers=sdk-plugin-runtime-boundary.inventory]
- [ ] [serial] I2: Define neutral manifest/tool runtime DTOs or owner modules that do not depend on TUI/protocol display types. r[sdk-plugin-runtime-boundary.neutral-contracts.no-display-dtos] [covers=sdk-plugin-runtime-boundary.neutral-contracts.no-display-dtos]
- [ ] [parallel] I3: Keep Extism, stdio, built-in, and product-owned runtime dispatch owners separate with fail-closed routing. r[sdk-plugin-runtime-boundary.dispatch.separate-owners] [covers=sdk-plugin-runtime-boundary.dispatch.separate-owners]
- [ ] [parallel] I4: Move or isolate plugin UI/status projection to desktop display adapters. r[sdk-plugin-runtime-boundary.neutral-contracts.ui-edge] [covers=sdk-plugin-runtime-boundary.neutral-contracts.ui-edge]

## Phase 2: Verification

- [ ] [serial] V1: Add or update plugin runtime dispatch matrix fixtures covering Extism, stdio, built-in, product-owned, forbidden-loader, and missing-policy cases. r[sdk-plugin-runtime-boundary.verification.dispatch-matrix] [covers=sdk-plugin-runtime-boundary.verification.dispatch-matrix]
- [ ] [serial] V2: Add source/dependency rails rejecting TUI/protocol imports from neutral plugin manifest/runtime modules. r[sdk-plugin-runtime-boundary.verification.boundary-rails] [covers=sdk-plugin-runtime-boundary.verification.boundary-rails]
- [ ] [serial] V3: Run plugin focused tests, dispatch checker, SDK dependency checks, Cairn gates/validate, and `git diff --check`. r[sdk-plugin-runtime-boundary.verification] [covers=sdk-plugin-runtime-boundary.verification]
