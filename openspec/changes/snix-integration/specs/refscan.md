# Store Reference Scanning

## Purpose

Scan tool output text for nix store path references and annotate them
with parsed metadata.  Helps the agent track which nix packages are
involved in build outputs, runtime closures, and error messages.

## Requirements

### Scanner function

r[nix.refscan.scan]
The system MUST provide a function to scan arbitrary text for store path
references:

```rust
pub fn scan_store_refs(text: &str) -> Vec<NixPath>;
```

This finds all substrings matching `/nix/store/<32-char-hash>-<name>`
and parses each into a `NixPath`.  Duplicates are deduplicated by path.

GIVEN text "error: collision between /nix/store/aaa-foo-1.0/bin/x and /nix/store/bbb-bar-2.0/bin/x"
WHEN `scan_store_refs` is called
THEN it returns two NixPaths: foo-1.0 and bar-2.0

GIVEN text with no store paths
WHEN `scan_store_refs` is called
THEN it returns an empty vec

### Wu-Manber acceleration (phase 3)

r[nix.refscan.wu-manber]
When the `refscan` feature is enabled, the system SHOULD use
`snix_castore::refscan::ReferencePattern` for faster multi-pattern scanning
when searching for known store paths in large outputs.

For the common case (finding any store path in output), the regex-based
scanner from phase 1 is sufficient.  Wu-Manber is only beneficial when
scanning for specific known paths in large (>1 MB) outputs.

### Annotation format

r[nix.refscan.annotate]
The system MUST provide a function to annotate tool output with a store
path summary:

```rust
pub fn annotate_store_refs(text: &str) -> Option<String>;
```

Returns `None` if no store paths found.  Otherwise returns a compact
summary suitable for appending to tool results:

```
[nix refs: glibc-2.38, gcc-13.3.0, hello-2.12.1 (3 store paths)]
```

### Post-processing integration

r[nix.refscan.post-process]
The agent SHOULD apply `annotate_store_refs` to tool outputs that are
likely to contain store paths:

- `bash` tool output
- `nix` tool output (already partially handled)
- `read` tool output when reading files under `/nix/store/`

This is advisory — the annotation is appended after the tool result,
not injected into it.  The agent can use it for context but it doesn't
alter the tool's output contract.

r[nix.refscan.opt-in]
Store path annotation MUST be opt-in, controlled by a setting in
clankers config:

```toml
[nix]
annotate_store_refs = true  # default: false
```

When disabled, no scanning occurs and no annotations are added.

### Performance

r[nix.refscan.perf]
Scanning MUST NOT add measurable latency to tool execution for typical
outputs (<100 KB).  The regex scanner uses a single pass over the text.
For outputs larger than 1 MB, scanning MAY be skipped entirely to avoid
stalling the agent turn loop.
