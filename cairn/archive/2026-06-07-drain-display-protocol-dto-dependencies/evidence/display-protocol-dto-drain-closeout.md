Artifact-Type: validation-log
Task-ID: R1,I1,I2,I3,V1,V2
Covers: r[remaining-coupling-drain.display-protocol-dependency-drain.inventory], r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos], r[remaining-coupling-drain.display-protocol-dependency-drain.protocol-edge], r[remaining-coupling-drain.display-protocol-dependency-drain.rails], r[remaining-coupling-drain.display-protocol-dependency-drain.validation], r[remaining-coupling-drain.display-protocol-dependency-drain.closeout]
Status: pass

## Current dependency inventory

Current non-dev workspace dependents after the drain:

```text
clanker-tui-types: ["clankers", "clankers-tui"]
clankers-protocol: ["clankers", "clankers-controller"]
```

The removed inward dependents were drained in focused slices:

- model-selection/procmon cost and process DTOs moved to `clanker-message`;
- util syntax highlighting now implements the canonical `rat-markdown` trait directly;
- TUI plugin summaries are projected from protocol DTOs at attach/event boundaries;
- plugin UI DTOs moved to `clanker-message` with compatibility reexports;
- config keymap/menu settings are data-only and projected at the root/TUI edge.

The remaining dependents are declared edge adapters:

- `clankers-tui` is the display crate and owns display DTO consumption;
- `clankers` is the root shell that adapts display and transport contracts;
- `clankers-controller` owns controller/transport protocol conversion seams, with constructor ownership guarded by FCIS.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clanker-tui-types -p clankers-plugin -p clankers-tui -p clankers-config -p clankers-controller -p clankers --tests
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
cargo test -p clankers-controller --test fcis_shell_boundaries
nix run .#cairn -- validate --root .
nix run .#cairn -- gate tasks drain-display-protocol-dto-dependencies --root .
git diff --check
```

All commands exited 0.
