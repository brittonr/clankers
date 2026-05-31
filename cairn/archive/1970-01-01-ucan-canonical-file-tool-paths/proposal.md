# Proposal: UCAN Canonical File Tool Paths

## Why

The public UCAN + Basalt gate now requires file-oriented tools to provide an explicit `path`, but the resource is still built from the raw tool input string. Relative paths such as `src/lib.rs` and traversal attempts such as `../secret` need deterministic remote-session semantics before they become `clankers:file:...` authorization resources.

Remote auth should authorize the same file resource that the tool will use. Relative paths must resolve against the session/project working directory, and parent traversal that escapes that root must deny before Basalt admission or tool execution.

## What Changes

- Add a session/project file-root context to the public UCAN tool authorization path.
- Resolve relative file-tool paths against that root before building `file/read` or `file/write` admission requests.
- Deny parent traversal escapes and malformed paths without using filesystem existence as an authority oracle.
- Preserve absolute-path behavior and legacy local `settings.defaultCapabilities` behavior.
- Add focused tests and a deterministic checker receipt for the canonicalization contract.

## Impact

- **Files**: expected touch points are `src/capability_gate.rs`, public-auth session construction in daemon/Matrix paths, one checker under `scripts/`, and Cairn lifecycle artifacts.
- **Testing**: focused capability-gate tests for relative resolution, traversal denial, absolute-path behavior, and legacy-gate non-regression; checker receipt; Cairn validation/gates.
