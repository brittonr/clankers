Evidence-ID: agent-provider-neutral-dtos-validation
Task-ID: V1,V2,V3
Artifact-Type: command-log
Covers: agent-provider-neutral-dtos.verification.import-rail, agent-provider-neutral-dtos.model-adapter.turn-policy, agent-provider-neutral-dtos.verification.closeout
Status: complete

# Agent Provider Neutral DTO Validation

## Source-boundary rail

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-lego-architecture-boundaries.rs
```

Result: exit status 0.

Relevant output:

```text
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json
```

The rail now records `agent_provider_neutral_dtos` in `policy/lego-architecture/dependency-ownership-baseline.json`, rejects `clankers_provider::message`, `clankers_provider::streaming`, `clankers_provider::Usage`, and `clankers_provider::ThinkingConfig` in reusable agent policy modules, and names `clanker-message` as the neutral DTO owner.

## Focused agent turn and compaction tests

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent turn
```

Result: 88 tests run, 88 passed, 101 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent compaction
```

Result: 23 tests run, 23 passed, 166 skipped.

## Compile check

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers-agent --tests
```

Result: exit status 0.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --no-run
```

Result: exit status 0.

## Closeout checks

```text
nix run .#cairn -- gate proposal agent-provider-neutral-dtos --root .
nix run .#cairn -- gate design agent-provider-neutral-dtos --root .
nix run .#cairn -- gate tasks agent-provider-neutral-dtos --root .
```

Result: all three gates returned `valid: true` and `verdict: PASS`.

```text
nix run .#cairn -- validate --root .
```

Result: `valid: true`; 6 active changes and 57 specs validated.

```text
git diff --check
```

Result: exit status 0.
