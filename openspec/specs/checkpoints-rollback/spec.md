# checkpoints-rollback Specification

## Purpose
Define the local git-backed working-directory checkpoint and rollback capability, including safe first-pass boundaries, session metadata, and verification expectations.

## Requirements

### Requirement: Working Directory Checkpoints and Rollback Capability [r[checkpoints-rollback.capability]]
The system MUST snapshot repository state before agent file changes and provide an explicit rollback command.

#### Scenario: Primary path succeeds [r[checkpoints-rollback.scenario.primary-path]]
- GIVEN clankers runs in a local git checkout
- WHEN the user or agent invokes the documented checkpoint create/list/rollback path
- THEN clankers performs the operation with the local git-backed checkpoint store and returns a structured, user-visible result

#### Scenario: Unsupported configuration is explicit [r[checkpoints-rollback.scenario.unsupported-config]]
- GIVEN the user invokes the capability outside the supported first-pass local git boundary
- WHEN clankers cannot safely proceed, including non-git directories, unsupported remote stores, submodule recursion, or rollback without explicit confirmation
- THEN clankers MUST return an actionable error instead of silently falling back or dropping work

### Requirement: Working Directory Checkpoints and Rollback Session Observability [r[checkpoints-rollback.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets or raw repository changes.

#### Scenario: Session records useful metadata [r[checkpoints-rollback.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes action, status, backend identity, repository root, checkpoint id when available, affected counts when useful, and redacted error details when applicable
- AND the session record MUST NOT include raw diffs, file contents, credentials, environment variables, or connection strings

### Requirement: Working Directory Checkpoints and Rollback Verification [r[checkpoints-rollback.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[checkpoints-rollback.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure
