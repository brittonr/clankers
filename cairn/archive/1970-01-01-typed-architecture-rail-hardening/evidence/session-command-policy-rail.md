Evidence-ID: typed-architecture-rail-hardening-session-command-policy
Task-ID: V1,V2
Artifact-Type: command-log
Covers: typed-architecture-rail-hardening.anchor-inventory, typed-architecture-rail-hardening.typed-checks, typed-architecture-rail-hardening.diagnostics, typed-architecture-rail-hardening.verification
Status: complete

# Typed Architecture Rail Hardening — Session Command Policy Cluster

## Selected anchor cluster

Selected `scripts/check-lego-architecture-boundaries.rs::session_command_policy_signature()` because it recently guarded the neutral display/protocol effects drain with exact source-string anchors.

## Replaced exact-string checks

The selected cluster now validates ownership with typed Rust AST inventories instead of exact source strings:

- enum variant inventory for `LocalSessionEffect`, `SessionAckPolicy`, and `SessionCommandIntent`;
- `SessionCommandEffect` field type inventory for `local`, `command`, and `ack`;
- effect-factory return type checks requiring `SessionCommandEffect`;
- function-body path checks for neutral intent variants and ack-policy variants;
- function-body path checks for `session_command_intent_to_protocol(...)` protocol projection ownership;
- use-path forbids rejecting `clankers_protocol::SessionCommand` in reusable policy;
- nested test fixture discovery through AST function inventory.

The generated baseline records `source_anchor_inventory` with classification `replaced`, so no exact-string fallback remains in the selected cluster.

## Diagnostic improvement

New failure labels include `source=...`, `target_owner=...`, and `replacement_path=...`, for example reusable policy failures now point from `src/modes/session_command_policy.rs` toward neutral `SessionCommandIntent` or the protocol projection owner `src/slash_commands/effects.rs::session_command_intent_to_protocol`.

## Validation

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --tests
```

Result: exit status 0.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers --test attach_parity_docs
```

Result: 4 tests run, 4 passed, 0 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-lego-architecture-boundaries.rs
```

Result: exit status 0; inventory written to `target/lego-architecture/dependency-ownership-inventory.json`.

```text
nix run .#cairn -- gate proposal typed-architecture-rail-hardening --root .
nix run .#cairn -- gate design typed-architecture-rail-hardening --root .
nix run .#cairn -- gate tasks typed-architecture-rail-hardening --root .
```

Result: all three gates returned `valid: true` and `verdict: PASS`.

```text
nix run .#cairn -- validate --root .
```

Result before archive: `valid: true`; 1 active change and 52 specs validated.

```text
nix run .#cairn -- archive typed-architecture-rail-hardening --root . --execute
nix run .#cairn -- validate --root .
```

Result after archive: archive returned `mutated: true`; validation returned `valid: true` with 0 active changes and 51 specs validated.

```text
git diff --check
```

Result: exit status 0.
