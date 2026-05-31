# Design: UCAN Canonical File Tool Paths

## Context

`PublicUcanCapabilityGate` maps remote public UCAN tool calls into `BasaltAdmissionRequest` values. File-oriented tools now require an explicit non-empty `path`, but `file_request(...)` still receives that raw value. If a remote model requests `grep` with `path: "src"`, the authorization resource should be the session-rooted path the tool will actually read, not an unrooted string.

The capability-gate trait currently receives only `(tool_name, input)`. The public auth object already carries session-specific metadata (`session_resource_id`) and is the right place to carry a session/project file root without changing the trait for local gates.

## Decisions

### 1. Carry a public file authorization root in the public auth object

**Choice:** Extend `PublicUcanToolAuthorization` with an optional `file_authority_root` (or equivalent) set during remote session construction from the session/project working directory. Public file-tool request construction must require this root before resolving relative paths.

**Rationale:** This keeps the stricter remote UCAN semantics inside the public gate and avoids changing the generic `CapabilityGate` trait for local/test-only gates. It also gives attach, Matrix, and keyed-session recovery one session-scoped fact to preserve.

### 2. Resolve relative paths syntactically, not through filesystem canonicalization

**Choice:** Add a pure path-normalization helper that combines `file_authority_root` with relative tool paths, folds `.` components, rejects NUL/empty paths, and denies parent traversal that would escape the root. It must not require the target file to exist, because write/edit tools may create or replace files.

**Rationale:** Filesystem `canonicalize` would deny valid future write targets and can follow symlinks or ambient filesystem state before authorization. The gate needs deterministic request facts, not a filesystem probe.

### 3. Preserve absolute-path and legacy-local behavior

**Choice:** Keep absolute paths normalized into `clankers:file:...` resources with the existing grant/policy decision deciding whether they are allowed. Keep `UcanCapabilityGate` unchanged for `settings.defaultCapabilities`.

**Rationale:** The small hardening slice should not change local compatibility behavior or reinterpret existing absolute-path grants. Parent traversal denial applies to rooted relative resolution; broader absolute-root restrictions can be a later policy slice if needed.

### 4. Verify path contracts at the request-construction seam

**Choice:** Test the helper/public request seam directly and through `PublicUcanCapabilityGate`, then add a deterministic checker that looks for the helper, root threading, tests, spec, and archived tasks.

**Rationale:** The risky behavior is request-shape drift before Basalt. Focused tests should prove the exact resource string, the fail-closed denial, and unchanged local legacy behavior without live credentials or network services.

## Risks / Trade-offs

- Remote sessions must thread a file root into public auth consistently across QUIC, Matrix, chat/RPC, and keyed-session recovery paths.
- Pure syntactic normalization does not resolve symlinks. That is acceptable for this slice because Basalt/UCAN grants are path vocabulary decisions; symlink containment policy belongs in a separate filesystem execution hardening layer.
- Absolute paths remain grant-controlled rather than root-confined to avoid widening the scope of this change.
