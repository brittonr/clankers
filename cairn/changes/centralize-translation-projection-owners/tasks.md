## Phase 1: Implementation

- [ ] [serial] I1: Inventory current projection/constructor owner modules for core, transport, display, provider, and session DTO families. r[remaining-coupling-drain.controller-command-seams.constructor-owners] [covers=remaining-coupling-drain.controller-command-seams.constructor-owners] [evidence=evidence/translation-projection-owners.md]
- [ ] [serial] I2: Extend FCIS or focused source rails to enforce constructor ownership for one additional DTO family. r[remaining-coupling-drain.controller-command-seams.constructor-owners] [covers=remaining-coupling-drain.controller-command-seams.constructor-owners] [evidence=evidence/translation-projection-owners.md]
- [ ] [serial] I3: Refactor touched call sites so reusable logic emits neutral DTOs and edge adapters project final shapes. r[remaining-coupling-drain.controller-command-seams.constructor-owners] [covers=remaining-coupling-drain.controller-command-seams.constructor-owners] [evidence=evidence/translation-projection-owners.md]

## Phase 2: Verification

- [ ] [serial] V1: Run FCIS shell-boundary tests plus focused daemon/attach/provider projection tests for the touched DTO family. r[remaining-coupling-drain.controller-command-seams.constructor-owners] [covers=remaining-coupling-drain.controller-command-seams.constructor-owners] [evidence=evidence/translation-projection-owners.md]
- [ ] [serial] V2: Run Cairn validation/gates, `git diff --check`, and relevant replay/parity acceptance rails. r[remaining-coupling-drain.controller-command-seams.constructor-owners] [covers=remaining-coupling-drain.controller-command-seams.constructor-owners] [evidence=evidence/validation-closeout.md]
