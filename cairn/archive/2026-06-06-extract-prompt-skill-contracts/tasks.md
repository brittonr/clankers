## Phase 1: Implementation

- [x] [serial] I1: Inventory runtime prompt/skill DTOs and decide the neutral owner boundary. r[remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [covers=remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [evidence=evidence/prompt-skill-contracts.md]
- [x] [serial] I2: Move neutral prompt/skill service contracts out of desktop path/config logic and wire runtime host injection through the new owner. r[remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [covers=remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [evidence=evidence/prompt-skill-contracts.md]
- [x] [serial] I3: Refresh embedded docs and runtime facade inventory after the split. r[remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [covers=remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [evidence=evidence/prompt-skill-contracts.md]

## Phase 2: Verification

- [x] [serial] V1: Run config/prompt/skill service fixtures and runtime fail-closed tests. r[remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [covers=remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [evidence=evidence/prompt-skill-contracts.md]
- [x] [serial] V2: Run Cairn validation/gates, `git diff --check`, and aggregate embedded SDK acceptance if public labels move. r[remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [covers=remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection] [evidence=evidence/validation-closeout.md]
