## ADDED Requirements

### Requirement: Controller continuation kit proves post-prompt state transitions [r[controller-continuation-policy.controller-continuation-policy-kit]]

The system MUST define `controller-continuation-policy-kit` as a composable Clankers brick with explicit ownership boundaries, deterministic fixtures, and safe evidence.

#### Scenario: Brick boundary is explicit [r[controller-continuation-policy.controller-continuation-policy-kit.boundary]]

- GIVEN a product or contributor adopts the `controller-continuation-policy-kit` brick
- WHEN the brick is documented, instantiated, or validated
- THEN the contract MUST name which behavior is reusable, which behavior stays product-owned, and which shell/runtime systems are out of scope
- THEN the brick MUST NOT silently depend on ambient credentials, daemon sessions, TUI state, provider discovery, plugin supervision, Matrix, iroh, or global singleton runtime state unless the design explicitly labels that path as app-edge

#### Scenario: Brick has executable evidence [r[controller-continuation-policy.controller-continuation-policy-kit.evidence]]

- GIVEN the brick is changed
- WHEN the focused verification for the change runs
- THEN it MUST exercise at least one positive path and one fail-closed or negative path through deterministic fixtures, examples, policy checks, generated inventory checks, or receipt validation
- THEN evidence MUST be safe to commit or summarize without raw prompts, credentials, authorization headers, OAuth tokens, provider payloads, hidden context, raw tool arguments, or secret environment values

#### Scenario: Brick drift is diagnosable [r[controller-continuation-policy.controller-continuation-policy-kit.drift]]

- GIVEN source code, docs, fixtures, policy, or generated inventories drift apart
- WHEN the brick validation rail runs
- THEN it MUST fail with a diagnostic that names the stale artifact and the expected owner of the update
- THEN intentional contract changes MUST require updating tests, docs, and receipt or fixture evidence together
