## Phase 1: Rail inventory and conversion plan

- [x] [serial] I1: Inventory every lego/SDK acceptance script and classify it as executable fixture, receipt verifier, AST/Cargo rail, or temporary string-presence check with owner and replacement path. [covers=r[behavioral-lego-parity-rails.inventory.classification]]
- [x] [serial] I2: Define a shared behavioral receipt schema with case id, axes, expected outcome, observed outcome, source artifacts, sanitized hashes, owner, and requirement ids. [covers=r[behavioral-lego-parity-rails.receipts.schema]]
- [x] [parallel] I3: Convert runtime extension service and shell adapter parity scripts from pure symbol checks into executable fixture/receipt verifiers. [covers=r[behavioral-lego-parity-rails.conversion.runtime-shell-matrices]]
- [ ] [parallel] I4: Add negative/fail-closed behavioral fixtures for provider/auth disabled defaults, missing session stores, denied capabilities, event redaction, and forbidden transport leakage. [covers=r[behavioral-lego-parity-rails.negative-fixtures.fail-closed]]
- [ ] [serial] I5: Wire converted receipts into `scripts/check-embedded-agent-sdk.rs` and Nix/check surfaces without depending on live credentials or local dotdirs. [covers=r[behavioral-lego-parity-rails.acceptance.wired-receipts]]

## Phase 2: Verification

- [ ] [parallel] V1: Add mutation or fixture-drift tests proving converted rails fail when expected behavior, axes, or source artifacts are missing. [covers=r[behavioral-lego-parity-rails.verification.rail-failure-fixtures]]
- [ ] [serial] V2: Run the embedded SDK acceptance bundle, converted rail scripts, Cairn validate/gates, and `git diff --check` before archive. [covers=r[behavioral-lego-parity-rails.verification.closeout]]
