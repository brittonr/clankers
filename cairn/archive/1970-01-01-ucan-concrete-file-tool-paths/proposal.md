# Proposal: UCAN Concrete File Tool Paths

## Why

Remote public UCAN + Basalt authorization currently maps file-tool operations to file resources only when the tool input includes a `path` field. Some read-only tools, such as `grep`, `find`, and `ls`, can default to the current working directory when `path` is omitted. Under a remote public UCAN gate, that omission leaves the invocation less concrete than the accepted daemon-auth vocabulary requires.

A remote credential should authorize the exact file resource being read or written. If a file-oriented tool defaults to an ambient directory, the gate should fail closed and require the caller to provide an explicit path before the tool can execute.

## What Changes

- Require public UCAN-gated file read/write tools to include a non-empty concrete `path` before constructing file read/write authorization requests.
- Preserve existing legacy/local `settings.defaultCapabilities` behavior; this hardening applies to the public UCAN + Basalt remote gate.
- Add focused tests and a deterministic checker receipt for the concrete path requirement.

## Impact

- **Files**: `src/capability_gate.rs`, `scripts/check-ucan-concrete-file-tool-paths.rs`, Cairn lifecycle artifacts.
- **Testing**: focused root capability-gate tests, deterministic checker, Cairn validation/gates.
