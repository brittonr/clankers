# Focused validation evidence

Evidence-ID: focused-validation
Artifact-Type: command-output-summary
Task-ID: V3
Covers: neutral-tool-service-context.verification
Date: 2026-06-01
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-tool-host
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers-agent -p clankers-tool-host --tests
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --no-run
./scripts/check-lego-architecture-boundaries.rs
nix run .#cairn -- gate design neutral-tool-service-context --root .
nix run .#cairn -- gate tasks neutral-tool-service-context --root .
nix run .#cairn -- validate --root .
rustfmt --check --config skip_children=true crates/clankers-agent/src/tool.rs crates/clankers-agent/src/turn/execution.rs crates/clankers-agent/src/turn/ports.rs crates/clankers-tool-host/src/lib.rs
git diff --check
```

## Relevant output

```text
clankers-agent: 193 tests run: 193 passed, 0 skipped
clankers-tool-host: 14 tests run: 14 passed, 0 skipped
cargo check clankers-agent + clankers-tool-host: Finished dev profile
cargo test -p clankers --no-run: Finished test profile
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json
Cairn design gate: PASS
Cairn tasks gate: PASS
Cairn validate: valid true
rustfmt --check: 0
git diff --check: 0
```
