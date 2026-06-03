Task-ID: I1,I2,I3,I4,V1,V2,V3
Covers: sdk-runtime-facade-split.inventory,sdk-runtime-facade-split.kits.selected-kit,sdk-runtime-facade-split.kits.independent-consumption,sdk-runtime-facade-split.inventory.support-labels,sdk-runtime-facade-split.verification.dependency-checks,sdk-runtime-facade-split.verification.fail-closed,sdk-runtime-facade-split.verification
Artifact-Type: validation-evidence

# Runtime Facade Split Closeout

## Selected kit

Selected kit: `session-ledger-resume`.

Public boundary:

- `SessionLedgerEntry`
- `SessionLedgerMessage`
- `SessionLedgerRecord`
- `SessionLedgerReplay`
- `Runtime::resume_session`
- `SessionStore` / `RuntimeServices` injection seam

Out-of-scope surfaces for this kit: provider/router/auth/plugin/TUI/daemon/process/Steel surfaces unless explicitly injected by the host.

## Inventory / labels

`scripts/check-runtime-facade-split.rs` classifies:

- `crates/clankers-runtime/src/lib.rs` as a yellow composition facade with selected green kit reexports.
- `crates/clankers-runtime/src/ledger.rs` as neutral green session ledger DTOs.
- `crates/clankers-runtime/src/session.rs` as host-owned resume execution for the selected kit.
- `crates/clankers-runtime/src/services.rs` as injected host services with disabled defaults that fail closed.
- `docs/src/tutorials/embedded-agent-sdk.md` as the docs support-label owner.

## Validation

Focused rails/tests:

- `nix develop -c cargo -q -Zscript scripts/check-runtime-facade-split.rs`
- `nix develop -c cargo -q -Zscript scripts/check-session-resume-brick.rs`
- `nix develop -c cargo -q -Zscript scripts/check-embedded-sdk-deps.rs`

