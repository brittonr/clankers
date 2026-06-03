Task-ID: I1,I2,I3,I4,V1,V2,V3
Covers: sdk-plugin-runtime-boundary.inventory,sdk-plugin-runtime-boundary.neutral-contracts.no-display-dtos,sdk-plugin-runtime-boundary.dispatch.separate-owners,sdk-plugin-runtime-boundary.neutral-contracts.ui-edge,sdk-plugin-runtime-boundary.verification.dispatch-matrix,sdk-plugin-runtime-boundary.verification.boundary-rails,sdk-plugin-runtime-boundary.verification
Artifact-Type: validation-evidence

# Plugin Runtime Boundary Closeout

## Inventory / owners

`scripts/check-plugin-runtime-boundary.rs` records responsibility owners:

- `manifest.rs`: neutral manifest schema/validation owner.
- `stdio_protocol.rs`: neutral stdio runtime protocol DTO owner.
- `host_facade.rs`: runtime inventory/query facade owner.
- `stdio_runtime.rs`: stdio supervisor/runtime dispatch owner.
- `ui.rs`: desktop UI projection edge where `clanker-tui-types` remains allowed.
- `scripts/check-plugin-runtime-dispatch.rs`: dispatch matrix rail for Extism, stdio, built-in, and product-owned runtime kinds.

## Dispatch matrix

`scripts/check-plugin-runtime-dispatch.rs` keeps Extism, stdio, built-in, and product-owned runtime dispatch owners separate and verifies forbidden-loader cases such as `stdio_sent_to_wasm_loader` fail closed in the fixture matrix.

## Validation

Focused rails/tests:

- `nix develop -c cargo -q -Zscript scripts/check-plugin-runtime-boundary.rs`
- `nix develop -c cargo -q -Zscript scripts/check-plugin-runtime-dispatch.rs`
- `nix develop -c cargo nextest run -p clankers-plugin plugin_runtime_dispatch_kit`

