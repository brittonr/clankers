Evidence-ID: steel-tool-plugin-substrate.V7.final
Task-ID: V7
Artifact-Type: deterministic-proof
Covers: steel-tool-plugin-substrate.verification.boundary-rail, steel-tool-plugin-substrate.verification.runtime-dogfood
Created-By: pi
Created-At: 2026-05-29T00:00:00Z

# V7 Final Validation Evidence

Commands:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --tests
mdbook build docs
nix run .#cairn -- gate proposal steel-tool-plugin-substrate --root .
nix run .#cairn -- gate design steel-tool-plugin-substrate --root .
nix run .#cairn -- gate tasks steel-tool-plugin-substrate --root .
nix run .#cairn -- validate --root .
git diff --check
git diff --cached --check
```

Observed results:

```text
cargo check -p clankers --tests: Finished dev profile
mdbook build docs: HTML book written to docs/book
proposal gate: PASS
design gate: PASS
tasks gate: PASS
cairn validate: valid true
git diff checks: no output
```

This evidence closes the Cairn package with successful compile, docs, proposal/design/tasks gates, repo Cairn validation, and whitespace checks.
