# Tasks: Steel Turn Planning UCAN Authority

## Implementation

- [ ] [serial] I1. Define the Steel turn-planning UCAN authority DTOs, stable ability/resource vocabulary, and redacted receipt shape. [r[steel-turn-planning-ucan-authority.vocabulary], r[steel-turn-planning-ucan-authority.receipts]]
- [ ] [serial] I2. Add a Rust-owned authority adapter that evaluates `steel.host.plan_turn` invocation decisions through the UCAN adapter seam after profile/script validation and before Steel execution. [r[steel-turn-planning-ucan-authority.adapter], r[steel-turn-planning-ucan-authority.evaluation-order]]
- [ ] [serial] I3. Thread the adapter into normal and orchestrated Steel turn-planning activation without changing disabled-by-default behavior. [r[steel-turn-planning-ucan-authority.evaluation-order], r[steel-turn-planning-ucan-authority.no-ambient-authority]]
- [ ] [serial] I4. Emit deterministic allowed/denied authority receipts that are daemon/session visible where Steel planning receipts are visible. [r[steel-turn-planning-ucan-authority.receipts]]
- [ ] [serial] I5. Add focused docs and a checker receipt under `target/steel-turn-planning-ucan-authority/`. [r[steel-turn-planning-ucan-authority.verification]]

## Verification

- [ ] [serial] V1. Add positive tests proving a matching UCAN grant allows the reviewed Steel planner and still uses the Rust-owned provider path. [r[steel-turn-planning-ucan-authority.adapter.allowed], r[steel-turn-planning-ucan-authority.verification.tests]]
- [ ] [serial] V2. Add negative tests for missing, expired, revoked, wrong-audience, wrong-resource, wrong-ability, unknown caveat, and overbroad grant denial before Steel/provider/tool execution. [r[steel-turn-planning-ucan-authority.adapter.denied], r[steel-turn-planning-ucan-authority.no-ambient-authority], r[steel-turn-planning-ucan-authority.verification.tests]]
- [ ] [serial] V3. Verify receipt redaction excludes raw compact UCAN tokens, signing material, prompts, provider payloads, profile bodies, and script bodies. [r[steel-turn-planning-ucan-authority.receipts.redacted], r[steel-turn-planning-ucan-authority.verification.checker]]
- [ ] [serial] V4. Run focused Rust tests/checks, the new checker, Cairn validation/gates, and `git diff --check`. [r[steel-turn-planning-ucan-authority.verification]]
- [ ] [serial] V5. Sync/archive the Cairn change, inspect accepted spec durability, restore any sync-truncated accepted spec, and land clean pushed `main`. [r[steel-turn-planning-ucan-authority.archive]]
