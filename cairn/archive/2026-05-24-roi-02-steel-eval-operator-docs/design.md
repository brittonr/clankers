# Design: Steel eval operator documentation

## Context

The `steel_eval` tool is now default-published, but operator-facing docs do not yet make the default tool surface, opt-out, authority boundary, and receipt review path easy to discover.

## Approach

- Keep this as a narrow documentation/catalog sync against existing behavior.
- Update README/reference docs where users already look for built-in tools and Steel runtime behavior.
- Verify with targeted greps, `git diff --check`, and the cheapest doc-adjacent metadata/check command available.

## Verification

- Validate this Cairn package with repo-local/native Cairn validation.
- Run proposal, design, and tasks gates and inspect `valid`/`verdict` receipts.
- Run the implementation-specific verification named in `tasks.md` when draining this package.
