## Phase 1: Implementation

- [ ] [serial] I1: Define the workspace layer map and allowed adapter exceptions in a deterministic policy/inventory source. r[remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [covers=remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [evidence=evidence/workspace-layering-rails.md]
- [ ] [serial] I2: Implement a Cargo metadata plus AST-backed rail that detects forbidden upward edges and constructor ownership drift. r[remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [covers=remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [evidence=evidence/workspace-layering-rails.md]
- [ ] [serial] I3: Replace or narrow at least one brittle source-string boundary check with the generated layer rail. r[remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [covers=remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [evidence=evidence/workspace-layering-rails.md]

## Phase 2: Verification

- [ ] [serial] V1: Run the new layering rail, FCIS shell-boundary tests, and existing embedded SDK dependency rails. r[remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [covers=remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [evidence=evidence/workspace-layering-rails.md]
- [ ] [serial] V2: Run Cairn validation/gates, `git diff --check`, and the broader architecture acceptance bundle. r[remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [covers=remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map] [evidence=evidence/validation-closeout.md]
