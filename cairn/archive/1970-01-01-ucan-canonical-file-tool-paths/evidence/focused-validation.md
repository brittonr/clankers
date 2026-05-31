Evidence-ID: focused-validation
Artifact-Type: validation-log
Task-ID: V1
Covers: r[ucan-basalt-daemon-auth.verification.canonical-file-path-tests]
Status: pass

# Focused Validation

## Command

`TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib capability_gate`

## Result

Pass. The focused root library filter ran 34 capability-gate related tests; 34 passed, 0 failed, 0 ignored, and 989 tests were filtered out.

## Coverage Notes

- `public_ucan_relative_file_path_resolves_under_file_root` proves `src/lib.rs` resolves to `clankers:file:/workspace/project/src/lib.rs` under a session file root and authorizes through `PublicUcanCapabilityGate`.
- `public_ucan_relative_file_path_requires_file_root_and_denies_escape` proves relative paths require a file root and `../secret` denies before Basalt/tool execution.
- `public_ucan_absolute_file_path_keeps_explicit_resource_semantics` proves absolute paths keep their explicit resource semantics.
- `legacy_ucan_gate_preserves_tool_only_default_capabilities_behavior` remains green, proving local legacy `settings.defaultCapabilities` behavior was not changed.
