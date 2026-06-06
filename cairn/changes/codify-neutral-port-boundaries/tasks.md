## Phase 1: Implementation

- [ ] [serial] I1: Inventory existing port traits, DTOs, and concrete adapters across agent/controller/runtime seams. r[remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [covers=remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [evidence=evidence/neutral-port-boundaries.md]
- [ ] [serial] I2: Document the neutral-port rule in embedded or architecture docs and identify the first seam to enforce. r[remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [covers=remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [evidence=evidence/neutral-port-boundaries.md]
- [ ] [serial] I3: Refactor the chosen seam so reusable policy uses injected ports and concrete behavior stays in adapters. r[remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [covers=remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [evidence=evidence/neutral-port-boundaries.md]

## Phase 2: Verification

- [ ] [serial] V1: Run focused seam tests plus dependency-boundary and FCIS rails for the touched component. r[remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [covers=remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [evidence=evidence/neutral-port-boundaries.md]
- [ ] [serial] V2: Run Cairn validation/gates, `git diff --check`, and aggregate SDK acceptance if public labels move. r[remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [covers=remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule] [evidence=evidence/validation-closeout.md]
