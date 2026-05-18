# deterministic-agent-testing Specification

## Purpose

Define Clankers' credential-free deterministic agent-turn replay contract for scripted provider responses, scripted tool results, normalized replay artifacts, and BLAKE3-bound regression receipts.

## Requirements

### Requirement: Deterministic turn replay fixtures

Clankers MUST provide credential-free deterministic agent-turn replay fixtures that describe fixed user input, session metadata, scripted model responses, expected tool calls/results, and expected normalized outputs for replayable model/tool turns. The replay rail MUST include both the pure engine reducer boundary and at least one controller/agent shell boundary that constructs provider requests and dispatches tool calls without live provider credentials, network access, daemon sockets, or ambient user config.

#### Scenario: fixture describes a complete tool turn

- GIVEN a deterministic replay fixture with a fixed session id, user prompt, scripted first model response requesting a tool, scripted tool result, and scripted final model response
- WHEN the replay test loads the fixture
- THEN it SHALL execute the turn without live provider credentials, network access, daemon sockets, or ambient user config
- AND it SHALL drive the same model→tool→model continuation shape described by the fixture

#### Scenario: fixture inputs are isolated from ambient state

- GIVEN a deterministic replay fixture
- WHEN the replay test runs
- THEN config, auth, session, cache, receipt, and tool filesystem state SHALL be isolated to test-owned temporary directories or in-memory stores
- AND the fixture SHALL provide any session id, clock, provider, and tool inputs needed for the replay contract

#### Scenario: controller replay crosses the shell boundary

- GIVEN a deterministic controller replay fixture with fixed session metadata, scripted provider responses, and scripted tool results
- WHEN the replay test runs through the controller or agent shell boundary
- THEN it SHALL complete one user → model tool-call → tool result → model final-answer turn without live credentials, network, daemon sockets, or ambient user config
- AND it SHALL assert stable session metadata, message ordering, provider request shape, and tool schema visibility

### Requirement: Provider request shape is pinned

The deterministic replay rail MUST assert provider request shape before accepting a replay as valid, including session metadata, message ordering, tool schema visibility, and continuation content after tool results. Controller/agent replay MUST additionally prove that shell-owned provider conversion does not drop user text, assistant tool-call context, tool-result content, or session metadata.

#### Scenario: first model request includes stable session and message context

- GIVEN a fixture starts a deterministic turn
- WHEN the fake provider receives the first completion request
- THEN the test SHALL assert the request contains the expected session id metadata
- AND the request SHALL contain the expected user message ordering and tool availability for that fixture

#### Scenario: continuation request includes tool results

- GIVEN the scripted first model response requests a tool and the fake tool returns a result
- WHEN the engine/controller asks the fake provider for the continuation
- THEN the recorded continuation request SHALL include the expected assistant tool-call context and tool result content
- AND it SHALL NOT drop branch summaries, compaction summaries, session metadata, or required provider extra parameters that are part of the fixture contract

#### Scenario: controller request conversion preserves semantic content

- GIVEN a controller replay fixture runs through the shell-owned provider conversion path
- WHEN the fake provider records initial and continuation requests
- THEN the recorded requests SHALL include non-empty provider-native messages in the expected order
- AND the recorded requests SHALL preserve the expected session id or session metadata required by the fixture
- AND the continuation request SHALL include assistant tool-call context and tool-result content

### Requirement: Normalized replay output is byte-stable

The deterministic replay rail MUST normalize approved volatile fields and produce byte-stable replay artifacts for transcripts, event streams, provider requests, tool results, and receipts across both engine and controller replay rails.

#### Scenario: repeated replay produces identical artifacts

- GIVEN the same deterministic replay fixture is executed twice in separate isolated temp state
- WHEN both runs complete successfully
- THEN their normalized transcript, event stream, provider request, tool result, and receipt artifacts SHALL be byte-identical
- AND their BLAKE3 hashes SHALL match

#### Scenario: normalization is allowlisted

- GIVEN replay output contains volatile fields such as temp paths, timestamps, durations, process ids, or generated run directories
- WHEN the normalizer prepares artifacts for comparison
- THEN it SHALL replace only documented volatile fields with stable placeholders
- AND it SHALL preserve semantic fields such as session ids, message roles, tool names, tool inputs, tool outputs, provider request fields, errors, and terminal outcomes

#### Scenario: controller replay produces stable artifacts

- GIVEN the same deterministic controller replay fixture is executed twice in separate isolated temp state
- WHEN both runs complete successfully
- THEN their normalized transcript, event stream, provider request, tool result, and receipt artifacts SHALL be byte-identical
- AND their BLAKE3 hashes SHALL match

### Requirement: Negative and correlation cases are deterministic

The deterministic replay rail MUST include at least one negative or correlation edge-case fixture that proves mismatched feedback, failed tools, or invalid continuation input is rejected deterministically without corrupting replay state.

#### Scenario: mismatched feedback is rejected deterministically

- GIVEN a fixture supplies model or tool feedback with a correlation id that does not match pending replay state
- WHEN the replay rail applies that feedback
- THEN the system SHALL return an explicit deterministic rejection
- AND the normalized transcript and event artifacts SHALL show no state mutation after the rejected feedback except the recorded safe rejection event

#### Scenario: failed tool result remains replayable

- GIVEN a fixture scripts a tool failure
- WHEN the turn consumes that tool result
- THEN the replay rail SHALL record the failure with stable error class and redacted details
- AND repeated runs SHALL produce identical normalized artifacts and hashes
