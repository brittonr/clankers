# Tasks: Drain Display/Protocol DTO Dependencies

## Phase 1: Inventory

- [ ] [serial] R1: Inventory every non-edge dependency on `clanker-tui-types` and `clankers-protocol`, classifying each as display edge, transport edge, shared neutral DTO, or drain target. r[remaining-coupling-drain.display-protocol-dependency-drain.inventory] [covers=remaining-coupling-drain.display-protocol-dependency-drain.inventory]

## Phase 2: Implementation

- [ ] [serial] I1: Replace display-only DTO uses in reusable config, model-selection, procmon, util, plugin, controller, or root policy paths with neutral DTOs and display-edge adapters. r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos] [covers=remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos]
- [ ] [serial] I2: Keep protocol constructors and wire DTO projection in `convert.rs`, `transport_convert.rs`, or transport adapters only. r[remaining-coupling-drain.display-protocol-dependency-drain.protocol-edge] [covers=remaining-coupling-drain.display-protocol-dependency-drain.protocol-edge]
- [ ] [serial] I3: Harden dependency/source rails to fail on new inward display/protocol DTO dependencies outside declared edge adapters. r[remaining-coupling-drain.display-protocol-dependency-drain.rails] [covers=remaining-coupling-drain.display-protocol-dependency-drain.rails]
- [x] [serial] I4: Move model-selection cost contracts and procmon process observation contracts from `clanker-tui-types` into neutral `clanker-message` reexported by the TUI edge. r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos] [covers=remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos] [evidence=cairn/changes/drain-display-protocol-dto-dependencies/evidence/model-selection-procmon-neutral-dtos.md]
- [x] [serial] I5: Remove `clankers-util`'s `clanker-tui-types` dependency by implementing the canonical `rat-markdown` syntax highlighter contract directly. r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos] [covers=remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos] [evidence=cairn/changes/drain-display-protocol-dto-dependencies/evidence/util-syntax-highlighter-tui-type-drain.md]

## Phase 3: Verification

- [ ] [serial] V1: Run display/protocol DTO source rails, FCIS constructor-owner rail, attach/daemon projection tests, and neutral DTO focused tests for touched crates. r[remaining-coupling-drain.display-protocol-dependency-drain.validation] [covers=remaining-coupling-drain.display-protocol-dependency-drain.validation]
- [ ] [serial] V2: Run affected cargo checks, Cairn gates/validate, and `git diff --check` before closeout. r[remaining-coupling-drain.display-protocol-dependency-drain.closeout] [covers=remaining-coupling-drain.display-protocol-dependency-drain.closeout]
- [x] [serial] V3: Validate the neutral cost/process DTO slice with affected cargo checks plus lego/workspace layering rails. r[remaining-coupling-drain.display-protocol-dependency-drain.validation] [covers=remaining-coupling-drain.display-protocol-dependency-drain.validation] [evidence=cairn/changes/drain-display-protocol-dto-dependencies/evidence/model-selection-procmon-neutral-dtos.md]
- [x] [serial] V4: Validate the util syntax-highlighter display DTO drain with util/root cargo checks, no-run root tests, lego dependency ownership, and workspace layering rails. r[remaining-coupling-drain.display-protocol-dependency-drain.validation] [covers=remaining-coupling-drain.display-protocol-dependency-drain.validation] [evidence=cairn/changes/drain-display-protocol-dto-dependencies/evidence/util-syntax-highlighter-tui-type-drain.md]
