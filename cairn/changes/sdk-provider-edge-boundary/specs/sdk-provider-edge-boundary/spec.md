## ADDED Requirements

### Requirement: Provider concerns have single owners [r[sdk-provider-edge-boundary.concerns]]

Provider-native request shaping, auth refresh, account discovery, routing/fallback/cooldown, retry behavior, and stream normalization MUST each have one owner; compatibility layers MUST translate DTOs and errors only.

#### Scenario: duplicate abstractions are tracked [r[sdk-provider-edge-boundary.concerns.duplicate-abstractions]]
- GIVEN two provider request or stream abstractions remain in the workspace
- WHEN a request field, stream event, retry policy, or auth behavior changes
- THEN parity rails MUST prove the adapters remain synchronized
- OR the duplicate abstraction MUST be collapsed into the owning module

### Requirement: SDK model APIs are host-owned and neutral [r[sdk-provider-edge-boundary.neutral-model-api]]

Generic SDK model execution MUST use neutral model-host/runtime provider DTOs and MUST NOT require Clankers provider/router/auth/discovery crates.

#### Scenario: SDK host owns provider adapter [r[sdk-provider-edge-boundary.neutral-model-api.sdk-host-owned]]
- GIVEN an embedded product wants model execution
- WHEN it integrates a provider
- THEN it MUST implement `ModelHost` or an equivalent neutral runtime provider service
- AND it MUST NOT need `clankers-provider`, `clanker-router`, OAuth stores, provider discovery, or live Clankers credentials

#### Scenario: provider edge avoids display DTOs [r[sdk-provider-edge-boundary.neutral-model-api.no-display-dtos]]
- GIVEN provider-facing APIs express thinking, reasoning, tools, messages, or usage
- WHEN source-boundary rails inspect provider contracts
- THEN they MUST use neutral message/core/provider DTOs
- AND TUI/display-only types MUST remain at display or app-edge projection owners

### Requirement: Provider edge verification is fixture-backed [r[sdk-provider-edge-boundary.verification]]

Provider edge changes MUST be verified by literal fixtures, dependency rails, and adapter parity tests.

#### Scenario: literal fixtures pin request shape [r[sdk-provider-edge-boundary.verification.literal-fixtures]]
- GIVEN a provider adapter builds a request body
- WHEN tests compare expected JSON
- THEN expected fixtures MUST be explicit literals or checked external fixtures
- AND they MUST NOT be produced by calling the same builder under test

#### Scenario: dependency rails protect green SDK crates [r[sdk-provider-edge-boundary.verification.dependency-rails]]
- GIVEN generic SDK crates or examples are inspected
- WHEN dependency validation runs
- THEN provider/router/auth/discovery/TUI/protocol crates MUST be absent from their dependency graphs
