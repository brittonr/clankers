Artifact-Type: verification-note
Evidence-ID: engine-surface-rail
Task-ID: V1
Covers: embeddable.agent.engine.embeddingfocused.migrationrails.engineapirailsrejectappprotocolleakage, embeddable.agent.engine.appspecificconcerns.transportanduiconcernsstayappspecific

## Summary
Define a deterministic public-surface rail for the future `clankers-engine` crate that fails closed if app-shell protocol or UI types leak into exported engine signatures.

## Evidence
- Existing reference rail: `crates/clankers-controller/tests/fcis_shell_boundaries.rs`
- Existing no-std reference rail: `scripts/check-clankers-core-surface.sh`
- Proposed new rail target: `scripts/check-clankers-engine-surface.sh`
- Proposed inventory files: `crates/clankers-engine/src/lib.rs` plus any future public engine type modules

## Checks
- Reject app protocol types such as `clankers_protocol::DaemonEvent` and `clankers_protocol::SessionCommand` from the public engine surface.
- Reject TUI/runtime/widget imports such as `ratatui::`, `crossterm::`, and other terminal-shell types from the public engine surface.
- Reject direct transport/runtime concerns such as Tokio handles/channels, network types, filesystem handles, and interactive shell state from public engine API signatures.
- Allow only engine-native plain-data contracts plus reusable shell-agnostic message/content types that do not smuggle protocol semantics.
- Keep this rail separate from controller FCIS rails so the engine boundary stays enforceable even after controller/agent adapters are reshaped.

## Planned Command
- `./scripts/check-clankers-engine-surface.sh`
- `cargo test -p clankers-controller --test fcis_shell_boundaries clankers_engine_surface_stays_shell_native -- --nocapture`

## Expected Failure Mode
The rail must exit non-zero when any forbidden app-shell or runtime pattern appears in the exported `clankers-engine` surface.

## Current Implementation
- `crates/clankers-engine/src/lib.rs` now exists as the first workspace seam for host-facing engine-native contracts.
- `scripts/check-clankers-engine-surface.sh` fails closed on daemon, TUI, async-runtime, and other app-shell imports in the public engine surface.
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs::clankers_engine_surface_stays_shell_native` adds a deterministic repo-local rail that inventories engine source paths and rejects shell-native leakage.
