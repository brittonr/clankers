# durable-process-jobs Specification

## Requirements

### Requirement: Process job profile kit validates backend-neutral job manifests

The process-job-profile-kit SHALL resolve manifests into backend-neutral start requests without spawning a process.

#### Scenario: pure profile resolution
- GIVEN a valid profile manifest is selected
- WHEN resolving a profile produces a backend-neutral start request without spawning a process
- THEN safe profile identity metadata MUST be copied into the request.

#### Scenario: fail-closed profile policy
- GIVEN a disallowed backend, malformed command shape, secret-like environment key, resource limit above policy, disallowed cwd, disallowed writable path, or ambiguous manifest source is present
- WHEN profile validation runs
- THEN validation MUST fail closed before backend dispatch.

#### Scenario: writable path denial
- GIVEN a disallowed writable path appears in a manifest
- WHEN profile policy validates it
- THEN the disallowed writable path MUST be rejected.
