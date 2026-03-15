# NixTool Enhancements

## Purpose

Upgrade the existing NixTool (`src/tools/nix/`) to use clankers-nix for
typed parsing of inputs and outputs.  No new tools — just better behavior
from the existing one.

## Requirements

### Flake ref pre-validation

r[nix.tool.flakeref-prevalidate]
Before spawning the nix CLI, the NixTool MUST check whether any argument
looks like a flake reference and validate it via `clankers_nix::parse_flake_ref`:

```rust
for arg in &args {
    if looks_like_flake_ref(arg) {
        if let Err(e) = clankers_nix::parse_flake_ref(arg) {
            return ToolResult::error(format!("Invalid flake reference '{}': {}", arg, e));
        }
    }
}
```

r[nix.tool.looks-like-flakeref]
A string "looks like a flake ref" if it:
- Starts with `.#` or `./#`
- Starts with `github:`, `git+`, `path:`, `file+`, `sourcehut:`
- Contains `#` (fragment separator) with a valid-looking prefix

Arguments that don't look like flake refs are passed through unchanged.

### Structured output metadata

r[nix.tool.structured-output]
After a successful `nix build`, the tool MUST parse the output paths and
include them as structured data in the result:

Current format (unchanged, still present):
```
Build succeeded. Output paths:
/nix/store/abc123-hello-2.12.1
```

New addition (appended):
```
[build outputs]
  hello-2.12.1  /nix/store/abc123-hello-2.12.1
```

The structured section uses `clankers_nix::extract_store_paths` to parse
stdout lines from the build.

### Derivation info on build failure

r[nix.tool.drv-on-failure]
When a build fails and the error log contains a `.drv` path, the tool
MAY read and summarize the derivation to help the agent understand what
failed:

```
Build failed (exit code 1).

[derivation: hello-2.12.1]
  builder: /nix/store/...-bash-5.2/bin/bash
  system: x86_64-linux
  inputs: gcc-13.3.0, glibc-2.38, hello-2.12.1-src

Build log:
  ...
```

This is best-effort — if the `.drv` file doesn't exist locally (e.g.,
not yet fetched from a substituter), skip the derivation summary.

### No changes to streaming behavior

r[nix.tool.streaming-unchanged]
The `--log-format internal-json` streaming, progress display, and output
truncation MUST remain unchanged.  The nix-compat enhancements only affect
the final result formatting, not the streaming behavior during execution.

### No changes to sandboxing

r[nix.tool.sandbox-unchanged]
The Landlock sandbox applied to the nix child process MUST remain unchanged.
nix-compat parsing happens in-process after the child exits — it has no
effect on the sandbox.

### Backward compatibility

r[nix.tool.backward-compat]
All existing NixTool behavior MUST be preserved.  The enhancements are
additive — they append structured metadata to existing output, never
remove or alter the current format.  An agent that ignores the new fields
continues to work exactly as before.
