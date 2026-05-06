# Verification: expand-context-references

## Inventory

Touched seams for expanded context references:

- `crates/clankers-util/src/at_file.rs` — resolver policy, reference kinds, bounded git diff expansion, policy-gated URL fetch, safe metadata redaction, unit tests.
- `crates/clankers-util/Cargo.toml` / `Cargo.lock` — add `reqwest` blocking client for bounded URL fetches.
- `tests/context_references.rs` — integration coverage for file metadata persistence, fail-closed URL policy, git diff expansion, and policy-enabled URL fetch.
- `README.md` — user-facing supported reference kinds and safety policy.
- `docs/src/reference/request-lifecycle.md` — standalone/daemon expansion and metadata ownership.

## Verification

- `cargo fmt`
- `CARGO_TARGET_DIR=target cargo test -p clankers-util at_file -- --nocapture` — passed: 19 tests.
- `CARGO_TARGET_DIR=target cargo test --test context_references -- --nocapture` — passed: 4 tests.

## Safety notes

- URL references are disabled by default and return explicit unsupported receipts unless policy enables fetching.
- URL metadata sanitizes userinfo credentials in the raw reference before persistence/logging.
- Expanded content is injected into the prompt only; persisted metadata records kind/status/target/counts/errors, not fetched body text.
- Git diff expansion uses `git diff --no-ext-diff --no-color` and enforces a byte limit before injection.
- Session/artifact references remain explicit unsupported outcomes for this slice rather than silent drops.
