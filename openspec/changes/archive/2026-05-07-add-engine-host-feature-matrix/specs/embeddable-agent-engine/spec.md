## ADDED Requirements

### Requirement: Engine/host feature matrix coverage [r[embeddable-agent-engine.engine-host-feature-matrix]]
The system MUST verify the reusable engine and host runner with an explicit bounded matrix of interacting model, tool, retry, cancellation, usage, streaming, and budget features.

#### Scenario: matrix covers pairwise feature interactions [r[embeddable-agent-engine.engine-host-feature-matrix.pairwise]]
- GIVEN declared matrix axes for model mode, stop reason, tool behavior, retry behavior, cancellation timing, usage observation, stream validity, and request budget
- WHEN the matrix coverage checker runs
- THEN every axis value appears in at least one executed case
- THEN every pairwise interaction required by the matrix policy is covered by an executed case or an explicit documented exclusion

#### Scenario: critical triples protect known orchestration seams [r[embeddable-agent-engine.engine-host-feature-matrix.critical-triples]]
- GIVEN known-risk interactions such as streamed tool calls with usage, retryable failures with cancellation, and budget exhaustion after tool feedback
- WHEN the matrix runner executes critical cases
- THEN each critical interaction has a stable case ID and assertions over engine effects, correlated feedback, terminal outcome, and emitted events

#### Scenario: matrix remains provider-neutral [r[embeddable-agent-engine.engine-host-feature-matrix.provider-neutral]]
- GIVEN the matrix test suite runs in an environment with no provider credentials, router daemon, plugin runtime, or OAuth store
- WHEN engine/host matrix cases execute
- THEN they use fake host adapters only
- THEN no network, daemon autostart, credential lookup, or provider discovery occurs

#### Scenario: matrix failures identify axis values [r[embeddable-agent-engine.engine-host-feature-matrix.diagnostics]]
- GIVEN a matrix case fails
- WHEN the test report is inspected
- THEN it names the case ID, axis values, expected engine effects or terminal outcome, and observed divergence
