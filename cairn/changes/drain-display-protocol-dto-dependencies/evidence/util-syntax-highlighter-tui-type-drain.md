Artifact-Type: validation-log
Task-ID: I5,V4
Covers: r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos], r[remaining-coupling-drain.display-protocol-dependency-drain.validation]
Status: pass

## Scope

Removed the `clanker-tui-types` dependency from `clankers-util`:

- `SyntectHighlighter` now implements the canonical `rat_markdown::SyntaxHighlighter` trait directly.
- Highlight output now uses `rat_markdown::HighlightSpan` instead of the TUI-edge reexport path.
- TUI callers still receive the same trait object because `clanker-tui-types` reexports the `rat-markdown` trait for display-edge compatibility.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-util
cargo check -p clankers-util --tests
cargo check -p clankers
cargo check -p clankers --tests
cargo test -p clankers --no-run
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

The lego dependency ownership inventory now records `clanker-tui-types` with 4 internal dependents instead of 5. `clankers-util` is no longer one of the display DTO dependents.
