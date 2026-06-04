## Context

Existing usage comments describe modes, but there is no stable command that agents or operators can call to discover the current harness profile matrix. The harness receipt contract is already covered by dry-run tests.

## Decisions

### 1. Human-readable list output

**Choice:** Add a `list` mode that prints concise Markdown-like sections for modes, selectors, environment, and receipts.

**Rationale:** It is easy to read in terminals and easy to assert with substring-based contract tests without introducing another schema.

**Alternative:** Emit JSON. Rejected for this slice because the immediate need is operator discoverability; a JSON profile manifest can follow if automation needs structured selection.

### 2. Test the real shell entrypoint

**Choice:** Extend `tests/test_harness_contract.rs` to spawn `scripts/test-harness.sh list` and assert key mode/selector/env/receipt text.

**Rationale:** This keeps the shell wrapper honest while continuing to use Rust/nextest as the assertion owner.

## Risks / Trade-offs

- The first slice uses human-readable output, so tests should assert stable required terms rather than full text snapshots.
- The existing harness initializes receipt paths before mode dispatch; this slice keeps that behavior unchanged and focuses only on discoverability output.
