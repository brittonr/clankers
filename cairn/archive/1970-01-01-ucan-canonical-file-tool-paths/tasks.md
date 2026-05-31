# Tasks: UCAN Canonical File Tool Paths

## Phase 1: Implementation

- [x] [serial] I1. Extend the public UCAN tool authorization context with a session/project file root and thread it through remote session creation, attach recovery, Matrix/keyed sessions, and public capability-gate construction. [covers=r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.root-context]]
- [x] [serial] I2. Add a pure public file-path normalization helper that resolves relative paths under the file root, preserves absolute-path request semantics, rejects empty/NUL/malformed paths, and denies parent traversal escapes before Basalt admission. [covers=r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.relative-resolution], r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.traversal-denial], r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.absolute-paths]]
- [x] [serial] I3. Route public UCAN `file/read` and `file/write` request construction through the canonicalization helper while keeping legacy/local `UcanCapabilityGate` behavior unchanged. [covers=r[ucan-basalt-daemon-auth.tool-gate.canonical-file-tool-paths], r[ucan-basalt-daemon-auth.tool-gate.legacy-local-unchanged.canonical-file-paths]]

## Phase 2: Verification

- [x] [serial] V1. Add focused root/capability-gate tests proving `src/lib.rs` resolves under the session root, `../secret` denies before Basalt/tool execution, absolute paths keep existing request semantics, and local legacy gates remain unchanged. [covers=r[ucan-basalt-daemon-auth.verification.canonical-file-path-tests]] [evidence=evidence/focused-validation.md]
- [x] [serial] V2. Add and run a deterministic checker receipt for root threading, path normalization, traversal denial, absolute-path behavior, local-gate non-regression, spec, and tasks. [covers=r[ucan-basalt-daemon-auth.verification.canonical-file-path-checker]] [evidence=evidence/checker-validation.md]
- [x] [serial] V3. Run focused Rust tests, checker receipt, Cairn proposal/design/tasks gates, Cairn validation, sync/archive inspection, and diff checks before closeout. [covers=r[ucan-basalt-daemon-auth.verification.canonical-file-path-closeout]] [evidence=evidence/closeout-validation.md]
