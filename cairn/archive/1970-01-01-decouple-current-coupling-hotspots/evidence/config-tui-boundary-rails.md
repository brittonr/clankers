# Config/TUI boundary rail evidence

Evidence-ID: config-tui-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V1
Covers: coupling-hotspot-remediation.config-tui-boundary
Date: 2026-05-30
Status: PASS

## Commands

```text
./scripts/check-config-tui-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-config --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --lib
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib tui_config
```

## Relevant output

```text
ok: config/tui boundary rail passed
clankers-config: 66 passed; 0 failed
cargo check -p clankers --lib: Finished `dev` profile
clankers tui_config tests: 2 passed; 0 failed
```

## Coverage notes

The static rail checks that `crates/clankers-config/Cargo.toml` no longer depends on `clankers-tui`, `ratatui`, or `terminal-colorsaurus`; scans `crates/clankers-config/src` for forbidden display/projection tokens; and requires the new `src/tui_config.rs` projection adapter markers for theme loading/projection and keymap construction.

The config unit tests prove theme and keymap settings remain data-only serde structures. The root `tui_config` tests prove display projection still creates concrete TUI `Theme` and `Keymap` values at the product-shell edge.
