Evidence-ID: focused-runtime-smoke
Artifact-Type: test-report
Task-ID: V1
Covers: r[steel-executor-runtime-smoke.executor-visible.comparison], r[steel-executor-runtime-smoke.executor-visible.default]
Created: 2026-05-30
Status: complete

# Focused Runtime Smoke

## Commands

```text
rustfmt tests/embedded_controller.rs scripts/check-steel-turn-planning-runtime-smoke.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test -p clankers steel_runtime_smoke --test embedded_controller
./scripts/check-steel-turn-planning-runtime-smoke.rs
```

## Results

```text
running 5 tests
test steel_runtime_smoke_explicit_disable_keeps_rust_native ... ok
test steel_runtime_smoke_hash_mismatch_fails_closed_before_receipt ... ok
test steel_runtime_smoke_default_settings_emit_redacted_receipt ... ok
test steel_runtime_smoke_missing_authority_fails_closed_before_receipt ... ok
test steel_runtime_smoke_prompt_command_emits_redacted_receipt ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 32 filtered out
STATUS 0

./scripts/check-steel-turn-planning-runtime-smoke.rs
steel turn planning runtime smoke receipt written to target/steel-turn-planning-runtime-smoke/receipt.json
STATUS 0
```

The comparison smoke asserts `executor=RustNative`; the default-settings smoke asserts `executor=SteelScheme`. Both checks observe `DaemonEvent::SystemMessage` receipt text after a real controller `SessionCommand::Prompt`.
