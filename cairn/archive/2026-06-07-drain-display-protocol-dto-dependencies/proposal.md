# Change: Drain Display/Protocol DTO Dependencies

## Why

The architecture inventory still shows display/protocol DTO crates as widely depended on: `clanker-tui-types` is used by non-display crates such as config, model-selection, procmon, plugin, util, and TUI/root, while protocol DTOs are shared by controller and display edges. Some sharing is legitimate, but display/protocol types become coupling leaks when reusable policy depends on rendering or transport constructors.

## What Changes

- Inventory every non-edge dependency on `clanker-tui-types` and `clankers-protocol` and classify it as display edge, transport edge, shared neutral DTO, or drain target.
- Replace TUI-specific types in reusable config/model/procmon/util/plugin paths with neutral DTOs or edge adapters.
- Keep protocol frame and response constructors in `transport_convert.rs` or transport adapters, not in reusable domain policy.
- Harden typed rails so new inward display/protocol DTO dependencies fail with owner diagnostics.

## Impact

- **Files**: crates using `clanker-tui-types` or `clankers-protocol`, `crates/clankers-controller/src/{convert.rs,transport_convert.rs,domain_event.rs}`, `src/modes/attach/event_projection.rs`, architecture rails, and generated ownership receipts.
- **Testing**: display/protocol DTO source rail, FCIS constructor-owner rail, attach/daemon projection tests, config/model/procmon neutral DTO tests, `cargo check --tests`, Cairn gates, and diff checks.
