## ADDED Requirements

### Requirement: Supported tool service ports are dogfooded first [r[neutral-tool-context.supported-service-ports]]

A tool-host service/context API MUST NOT be promoted from experimental to supported until deterministic fixtures exercise positive and fail-closed behavior through the public API.

#### Scenario: promoted service port has positive and negative fixtures [r[neutral-tool-context.supported-service-ports.fixtures]]
- GIVEN a storage, search, hook, progress, capability, cancellation, or runtime-policy service API is promoted
- WHEN validation runs
- THEN fixtures MUST exercise the service through `ToolInvocationContext` or an equivalent neutral public API
- AND absent or denied service behavior MUST fail closed without constructing desktop defaults

#### Scenario: docs match promoted service semantics [r[neutral-tool-context.supported-service-ports.docs]]
- GIVEN a service/context API is classified as supported
- WHEN SDK docs are checked
- THEN the docs MUST describe host responsibilities, positive behavior, fail-closed behavior, and app-edge boundaries for that API
