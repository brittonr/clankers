Evidence-ID: neutral-display-protocol-effects-disabled-tools
Task-ID: V1,V2
Artifact-Type: command-log
Covers: neutral-display-protocol-effects.neutral-effects, neutral-display-protocol-effects.projection-adapters, neutral-display-protocol-effects.verification
Status: complete

# Neutral Display/Protocol Effects — Disabled Tools

## Implementation summary

- Selected disabled-tools slash/effect handling as the drained command family.
- Added neutral `SessionCommandIntent` in `src/modes/session_command_policy.rs` so reusable session policy returns neutral command intent data instead of constructing `SessionCommand::SetDisabledTools`.
- Added projection adapter `slash_commands::effects::session_command_intent_to_protocol(...)` for daemon protocol conversion at the edge.
- Updated attach disabled-tools paths to use `dispatch_disabled_tools_change(...)`, which applies local state, budgets acknowledgement suppression, and only then projects/sends daemon protocol.
- Updated standalone test interpreter projection and attach parity source rails to cover disabled-tools neutral intent projection.
- Updated lego architecture rail and baseline to reject `SessionCommand::SetDisabledTools` / `clankers_protocol::SessionCommand` returning to the selected policy owner.

## Focused slash/attach parity checks

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers disabled_tools
```

Result: 6 tests run, 6 passed, 1533 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers attach_tools_disable
```

Result: 2 tests run, 2 passed, 1537 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers --test attach_parity_docs
```

Result: 4 tests run, 4 passed, 0 skipped.

## Build and architecture rails

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --tests
```

Result: exit status 0.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-lego-architecture-boundaries.rs
```

Result: exit status 0; inventory written to `target/lego-architecture/dependency-ownership-inventory.json`.

```text
nix run .#cairn -- gate proposal neutral-display-protocol-effects --root .
nix run .#cairn -- gate design neutral-display-protocol-effects --root .
nix run .#cairn -- gate tasks neutral-display-protocol-effects --root .
```

Result: all three gates returned `valid: true` and `verdict: PASS`.

```text
nix run .#cairn -- validate --root .
```

Result before archive: `valid: true`; 2 active changes and 53 specs validated.

```text
nix run .#cairn -- archive neutral-display-protocol-effects --root . --execute
nix run .#cairn -- validate --root .
```

Result after archive: archive returned `mutated: true`; validation returned `valid: true` with 1 active change and 52 specs validated.

```text
git diff --check
```

Result: exit status 0.
