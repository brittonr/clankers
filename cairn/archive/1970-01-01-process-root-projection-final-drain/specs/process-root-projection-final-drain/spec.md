## ADDED Requirements

### Requirement: Root process tool is projection-only [r[process-root-projection-final-drain.root-projection]]

The root process tool MUST parse agent JSON into typed process-job requests, select a backend/service owner, and project typed receipts, but MUST NOT own native lifecycle state or backend-specific receipt policy.

#### Scenario: root file excludes native service state [r[process-root-projection-final-drain.root-projection.no-native-state]]
- GIVEN `src/tools/process.rs` is inspected
- WHEN native process jobs are started, listed, polled, logged, killed, restarted, adopted, or garbage-collected
- THEN the root file MUST delegate to a named process service owner
- AND the diagnostic MUST name the expected owner for any native service state found in the root file

### Requirement: Native service owner is complete [r[process-root-projection-final-drain.native-service-owner]]

Native process entry state, native service methods, native receipt helpers, and native service fixtures MUST live with the native process backend owner rather than in the root tool.

#### Scenario: native fixtures exercise owner directly [r[process-root-projection-final-drain.native-service-owner.direct-fixtures]]
- GIVEN native process behavior is verified
- WHEN focused tests run
- THEN tests MUST call the native service owner directly for native lifecycle behavior
- AND root-tool tests MUST be limited to parser, backend selection, and envelope projection parity

### Requirement: Final drain is guarded [r[process-root-projection-final-drain.verification]]

Verification MUST prove the root process tool stays projection-only and user-facing process behavior is preserved.

#### Scenario: rails reject root policy regression [r[process-root-projection-final-drain.verification.rails]]
- GIVEN the process root file changes
- WHEN boundary rails run
- THEN they MUST fail if native service structs, native entry state, or native lifecycle helpers return to `src/tools/process.rs`
