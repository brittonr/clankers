## Phase 1: Implementation

- [ ] [serial] I1: Build a command responsibility inventory over `command.rs` and identify one extraction cluster. r[controller-command-responsibility-drain.responsibility-map] [covers=controller-command-responsibility-drain.responsibility-map]
- [ ] [serial] I2: Extract the selected responsibility into a named controller module with a narrow public API. r[controller-command-responsibility-drain.single-purpose-module] [covers=controller-command-responsibility-drain.single-purpose-module]
- [ ] [serial] I3: Route command output through existing projection owners after extraction. r[controller-command-responsibility-drain.projection-owner] [covers=controller-command-responsibility-drain.projection-owner]
- [ ] [serial] I4: Update FCIS and lego rails to name the new owner and reject responsibility regression. r[controller-command-responsibility-drain.verification] [covers=controller-command-responsibility-drain.verification]

## Phase 2: Verification

- [ ] [serial] V1: Run focused controller tests for the moved command cluster. r[controller-command-responsibility-drain.verification] [covers=controller-command-responsibility-drain.verification]
- [ ] [serial] V2: Run FCIS shell boundary tests, `cargo check -p clankers-controller --tests`, Cairn gates/validate, and `git diff --check`. r[controller-command-responsibility-drain.verification] [covers=controller-command-responsibility-drain.verification]
