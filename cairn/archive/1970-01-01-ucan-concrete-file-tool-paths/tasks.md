# Tasks: UCAN Concrete File Tool Paths

## Phase 1: Implementation

- [x] [serial] I1. Add a public UCAN + Basalt request-construction helper that requires a non-empty concrete `path` for file read/write tools before any file `BasaltAdmissionRequest` is built. [covers=r[ucan-basalt-daemon-auth.vocabulary.concrete-file-paths], r[ucan-basalt-daemon-auth.tool-gate.concrete-file-paths]]
- [x] [serial] I2. Keep the legacy/local `UcanCapabilityGate` behavior unchanged while routing only `PublicUcanCapabilityGate` file-tool requests through the new concrete-path requirement. [covers=r[ucan-basalt-daemon-auth.tool-gate.legacy-local-unchanged]]

## Phase 2: Verification

- [x] [serial] V1. Add focused unit tests proving public UCAN-gated file tools deny omitted or blank paths and still build file read/write requests when a concrete path is present. [covers=r[ucan-basalt-daemon-auth.verification.concrete-file-path-tests]] [evidence=evidence/focused-validation.md]
- [x] [serial] V2. Add and run a deterministic checker receipt for the helper, tests, spec, tasks, and redaction-safe source markers. [covers=r[ucan-basalt-daemon-auth.verification.concrete-file-path-checker]] [evidence=evidence/checker-validation.md]
- [x] [serial] V3. Run Cairn validation/gates, sync/archive inspection, and diff checks before closeout. [covers=r[ucan-basalt-daemon-auth.verification.concrete-file-path-closeout]] [evidence=evidence/closeout-validation.md]
