# Tool catalog boundary rail evidence

Evidence-ID: tool-catalog-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V3
Covers: coupling-hotspot-remediation.tool-catalog-boundary
Date: 2026-05-30
Status: PASS

## Commands

```text
./scripts/check-tool-catalog-boundary.rs
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib tool_catalog
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib build_tiered_tools
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib build_all_tiered_tools
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --lib
```

## Relevant output

```text
ok: tool catalog boundary rail passed
modes::tool_catalog: 2 passed; 0 failed
build_tiered_tools: 8 passed; 0 failed
build_all_tiered_tools: 3 passed; 0 failed
cargo check -p clankers --lib: Finished `dev` profile
```

## Coverage notes

The static rail requires `src/modes/tool_catalog.rs` to declare named owners/builders for core, orchestration, specialty, daemon-session, matrix, plugin, extension-runtime, and MCP tool families. It also checks that runtime code in `src/modes/common.rs` delegates to the catalog owner and no longer owns concrete tool constructors such as built-in tool `::new` calls, plugin tool adapters, extension runtime builders, or MCP registration.

Focused root tests cover the owner inventory and expected family publications, then reuse the existing `build_tiered_tools` compatibility tests to prove callers still observe the same tool registrations for checkpoint, gateway, voice, soul, browser, Steel eval, and external memory paths.
