# Tasks: Steel Turn Planning Config Activation

## Config surface and loader

- [ ] [serial] I1: Add a stable typed settings surface for optional Steel turn planning activation, with missing config mapping to disabled/no Steel planning [r[steel-turn-planning-config-activation.settings-surface]] [r[steel-turn-planning-config-activation.settings-surface.absent-disabled]]
- [ ] [serial] I2: Implement Rust-owned profile/script loading and validation for reviewed `steel.host.plan_turn` profiles, including path, hash, budget, seam, rollout, fallback, and host-action checks [r[steel-turn-planning-config-activation.profile-loader]] [r[steel-turn-planning-config-activation.profile-loader.valid]]
- [ ] [parallel] I3: Add invalid-profile fixtures proving missing, malformed, over-budget, hash-mismatched, unsupported-seam, unsupported-host-action, and out-of-scope path data fail before Steel execution [r[steel-turn-planning-config-activation.profile-loader.invalid]] [r[steel-turn-planning-config-activation.fail-closed.unsupported-authority]]

## Real turn threading

- [ ] [serial] I4: Thread the optional activation result into normal and orchestrated real turn `TurnConfig` construction through one shared helper [r[steel-turn-planning-config-activation.turn-threading]] [r[steel-turn-planning-config-activation.turn-threading.normal]] [r[steel-turn-planning-config-activation.turn-threading.orchestrated]]
- [ ] [parallel] I5: Preserve existing comparison/default/fallback semantics from the Steel planning adapter and runtime when activated from config [r[steel-turn-planning-config-activation.fail-closed.fallback-policy]]

## Evidence, docs, and gates

- [ ] [parallel] D1: Document the config activation path, authority boundaries, profile fields, disabled/comparison/default behavior, and generated receipt location [r[steel-turn-planning-config-activation.verification]]
- [ ] [serial] G1: Add focused tests proving disabled/no-config, comparison, default, invalid profile/script/hash, and redacted deterministic receipt behavior [r[steel-turn-planning-config-activation.verification.tests]]
- [ ] [serial] G2: Add and run a deterministic checker that writes `target/steel-turn-planning-config-activation/receipt.json` without raw prompts, provider payloads, credentials, UCAN proofs, script bodies, or secret absolute paths [r[steel-turn-planning-config-activation.verification.checker]]
- [ ] [serial] G3: Run Cairn validate and proposal/design/tasks gates for `steel-turn-planning-config-activation` and inspect validity/verdict [r[steel-turn-planning-config-activation.verification]]
- [ ] [serial] G4: Run `git diff --check` before commit [r[steel-turn-planning-config-activation.fail-closed]]
