Artifact-Type: deterministic-proof
Task-ID: V1,V2,V3,V4
Covers: openspec-review-gates.metrics-derived-omission-prevention.task-fixtures, openspec-review-gates.deterministic-verification-tasks, openspec-review-gates.*

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-openspec-review-gates.rs
openspec validate roi-01-harden-openspec-gate-omission-prevention --strict --json
cargo fmt --check
mdbook build docs
git diff --check
nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).openspec-review-gates --no-link -L
```

## Result

PASS.

Notes:

- The Nix check fell back to local build after the known remote builder SSH failure for `ssh-ng://root@10.10.10.1`, then completed successfully.
- The review-gate checker verified negative and positive fixtures plus guidance/flake wiring.
- `openspec validate` for the active change returned `valid: true` before archive; `openspec validate openspec-review-gates --strict --json` returned `valid: true` after archive.
