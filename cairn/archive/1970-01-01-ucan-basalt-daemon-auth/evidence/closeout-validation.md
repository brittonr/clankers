Evidence-ID: closeout-validation
Artifact-Type: validation-report
Task-ID: V6
Covers: r[ucan-basalt-daemon-auth.verification.closeout]
Created: 2026-05-29
Updated: 2026-05-30
Status: complete

# Closeout Validation

## Commands

Successful validation commands run for this closeout:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-ucan --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --test auth_credential
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --test public_ucan_boundary
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib capability_gate
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers-ucan -p clankers-agent -p clankers-controller -p clankers --tests
git diff --check
nix run .#cairn -- validate --root .
nix run .#cairn -- gate proposal ucan-basalt-daemon-auth --root .
nix run .#cairn -- gate design ucan-basalt-daemon-auth --root .
nix run .#cairn -- gate tasks ucan-basalt-daemon-auth --root .
nix run .#cairn -- sync ucan-basalt-daemon-auth --root . --execute
nix run .#cairn -- archive ucan-basalt-daemon-auth --root . --execute
nix run .#cairn -- validate --root .
```

## Results

```text
cargo test -p clankers-ucan --lib: 86 passed; 0 failed
cargo test --test auth_credential: 10 passed; 0 failed
cargo test --test public_ucan_boundary: 3 passed; 0 failed
cargo test --lib capability_gate: 29 passed; 0 failed
cargo check touched crates --tests: Finished dev profile; Ok 0
git diff --check: Ok 0
cairn validate: valid true, changes 1
cairn proposal gate: PASS, receipt 0053574c8b5e1b2c8dfcf8ddc4dfc8fcb0924ca4da259b41b7b62d12a0ee050f
cairn design gate: PASS, receipt b75d45f23ec7f40e7eea6d5714a6cb299ac1c974f3b1494f61847aa5313dc2d6
cairn tasks gate: PASS, receipt d046244899b817280e76106a55c56d36fd00b5ec035de87917b918f32771f014
cairn sync --execute: mutated true, receipt 760620a1f8f489582507292bbc45b057dc40f372f5e899713ef70a19bbe35167
cairn archive --execute: mutated true, receipt 508a4b90e4143dbb793bb6a2d57f21212b55087ac30c0c633b07aa5a8b5b0daf
post-archive cairn validate: valid true, changes 0
post-archive git diff --check: Ok 0
```

## Post-I6 Re-run

Additional validation after extending call-time prompt/session/model checks:

```text
nix develop /home/brittonr/git/clankers -c cargo check --manifest-path /home/brittonr/git/clankers/Cargo.toml -p clankers-ucan -p clankers-agent -p clankers-controller -p clankers --tests
nix develop /home/brittonr/git/clankers -c cargo test --manifest-path /home/brittonr/git/clankers/Cargo.toml --test auth_credential --test public_ucan_boundary
nix develop /home/brittonr/git/clankers -c cargo test --manifest-path /home/brittonr/git/clankers/Cargo.toml -p clankers capability_gate --lib
nix develop /home/brittonr/git/clankers -c cargo test --manifest-path /home/brittonr/git/clankers/Cargo.toml -p clankers-controller capability_gate --lib
git diff --check
nix run .#cairn -- validate --root .
```

Result excerpts:

```text
cargo check touched crates --tests: Finished dev profile; Ok 0
auth_credential: 10 passed; 0 failed
public_ucan_boundary: 3 passed; 0 failed
clankers capability_gate: 29 passed; 0 failed
clankers-controller capability_gate: 3 passed; 0 failed
git diff --check: Ok 0
cairn validate: valid true, changes 0, specs_validated 45
```
