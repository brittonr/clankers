# Tasks: Drain Display/Protocol DTO Dependencies

## Phase 1: Inventory

- [ ] [serial] R1: Inventory every non-edge dependency on `clanker-tui-types` and `clankers-protocol`, classifying each as display edge, transport edge, shared neutral DTO, or drain target. r[remaining-coupling-drain.display-protocol-dependency-drain.inventory] [covers=remaining-coupling-drain.display-protocol-dependency-drain.inventory]

## Phase 2: Implementation

- [ ] [serial] I1: Replace display-only DTO uses in reusable config, model-selection, procmon, util, plugin, controller, or root policy paths with neutral DTOs and display-edge adapters. r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos] [covers=remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos]
- [ ] [serial] I2: Keep protocol constructors and wire DTO projection in `convert.rs`, `transport_convert.rs`, or transport adapters only. r[remaining-coupling-drain.display-protocol-dependency-drain.protocol-edge] [covers=remaining-coupling-drain.display-protocol-dependency-drain.protocol-edge]
- [ ] [serial] I3: Harden dependency/source rails to fail on new inward display/protocol DTO dependencies outside declared edge adapters. r[remaining-coupling-drain.display-protocol-dependency-drain.rails] [covers=remaining-coupling-drain.display-protocol-dependency-drain.rails]

## Phase 3: Verification

- [ ] [serial] V1: Run display/protocol DTO source rails, FCIS constructor-owner rail, attach/daemon projection tests, and neutral DTO focused tests for touched crates. r[remaining-coupling-drain.display-protocol-dependency-drain.validation] [covers=remaining-coupling-drain.display-protocol-dependency-drain.validation]
- [ ] [serial] V2: Run affected cargo checks, Cairn gates/validate, and `git diff --check` before closeout. r[remaining-coupling-drain.display-protocol-dependency-drain.closeout] [covers=remaining-coupling-drain.display-protocol-dependency-drain.closeout]
