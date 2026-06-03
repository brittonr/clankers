## Phase 1: Runtime facade split

- [ ] [serial] I1: Inventory `clankers-runtime` public modules/types and classify each as green SDK kit, yellow app-edge service, or red desktop compatibility. r[sdk-runtime-facade-split.inventory] [covers=sdk-runtime-facade-split.inventory]
- [ ] [serial] I2: Choose one runtime capability kit and define its public boundary, dependencies, defaults, and migration notes. r[sdk-runtime-facade-split.kits.selected-kit] [covers=sdk-runtime-facade-split.kits.selected-kit]
- [ ] [serial] I3: Extract, isolate, or feature-gate the selected kit so unrelated runtime surfaces are not required by consumers. r[sdk-runtime-facade-split.kits.independent-consumption] [covers=sdk-runtime-facade-split.kits.independent-consumption]
- [ ] [parallel] I4: Update generated SDK inventory/support labels and docs to reflect the kit boundary. r[sdk-runtime-facade-split.inventory.support-labels] [covers=sdk-runtime-facade-split.inventory.support-labels]

## Phase 2: Verification

- [ ] [serial] V1: Add dependency checks proving the selected kit avoids unrelated provider/router/auth/plugin/TUI/daemon/process/Steel surfaces unless explicitly in scope. r[sdk-runtime-facade-split.verification.dependency-checks] [covers=sdk-runtime-facade-split.verification.dependency-checks]
- [ ] [serial] V2: Add fail-closed tests for missing services, disabled filesystem/global discovery, and unsupported stores for the selected kit. r[sdk-runtime-facade-split.verification.fail-closed] [covers=sdk-runtime-facade-split.verification.fail-closed]
- [ ] [serial] V3: Run runtime focused tests, SDK API/dependency checks, example build/run for the selected kit, Cairn gates/validate, and `git diff --check`. r[sdk-runtime-facade-split.verification] [covers=sdk-runtime-facade-split.verification]
