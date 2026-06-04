# Context References API Surface

## User-facing surface

Context references are prompt syntax, not a separate built-in tool. Users type references directly in prompts:

- `@path/to/file` expands a UTF-8 text file into the prompt.
- `@path/to/file:42` expands a single line.
- `@path/to/file:10-20` expands an inclusive line range.
- `@path/to/dir/` expands a sorted directory listing.
- `@path/to/image.png` attaches an image block and leaves a short image label in the text where the caller supports image content.

The first-pass behavior is available through the same prompt-submission paths that already invoke `expand_at_refs_with_images`:

- interactive TUI prompt submission;
- daemon attach prompt submission;
- standalone/non-interactive prompt paths once wired in the implementation task.

## Structured internal surface

The implementation should expose a shared resolver API from `clankers-util`, preserving existing callers while adding testable policy and metadata boundaries:

- parse all candidate `@` references;
- resolve local filesystem references against the prompt cwd;
- return expanded text, image content blocks, and normalized per-reference metadata;
- report unsupported reference kinds as explicit, actionable replacement text and metadata instead of silently dropping them.

The first implementation can keep the legacy `expand_at_refs_with_images(text, cwd)` wrapper and implement richer behavior underneath so existing TUI/attach behavior remains compatible while new prompt paths can opt into metadata.

## Policy and unsupported cases

Supported in this change:

- local text files;
- local line ranges;
- local directory listings;
- local image references.

Explicitly unsupported in the first pass:

- `@http://...` and `@https://...` URL fetching;
- session artifact references;
- git diff references;
- remote or credential-bearing references;
- any reference that cannot be safely resolved as local filesystem content.

Unsupported references should be surfaced as actionable text such as `[Unsupported context reference @https://example.com: URL references are not supported yet]`. They should also produce metadata with status `unsupported` when the caller records metadata.

## Configuration surface

No durable config knob is required for the first pass because the feature already exists implicitly for local filesystem references. The implementation should keep local expansion deterministic and bounded by existing read limits, then reserve a future `contextReferences` config block for remote URLs, git-diff selectors, session artifact access, or larger budgets.

## Observability surface

The resolver should produce safe metadata for each reference:

- source: `context_references`;
- raw reference text;
- kind: `file`, `directory`, `image`, `unsupported`, or `error`;
- status: `expanded`, `unsupported`, or `error`;
- safe path or scheme/type information;
- line range when supplied;
- byte/line counts when available;
- redacted error text when applicable.

Persisting this metadata belongs to the later session-observability task; this task only fixes the API shape and user-facing policy.
