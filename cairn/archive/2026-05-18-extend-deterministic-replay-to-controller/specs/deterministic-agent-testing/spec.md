## MODIFIED Requirements

### Requirement: Deterministic turn replay fixtures

Clankers MUST provide credential-free deterministic agent-turn replay fixtures that describe fixed user input, session metadata, scripted model responses, expected tool calls/results, and expected normalized outputs for replayable model/tool turns. The replay rail MUST include both the pure engine reducer boundary and at least one controller/agent shell boundary that constructs provider requests and dispatches tool calls without live provider credentials, network access, daemon sockets, or ambient user config.

#### Scenario: controller replay crosses the shell boundary

- GIVEN a deterministic controller replay fixture with fixed session metadata, scripted provider responses, and scripted tool results
- WHEN the replay test runs through the controller or agent shell boundary
- THEN it SHALL complete one user → model tool-call → tool result → model final-answer turn without live credentials, network, daemon sockets, or ambient user config
- AND it SHALL assert stable session metadata, message ordering, provider request shape, and tool schema visibility

### Requirement: Provider request shape is pinned

The deterministic replay rail MUST assert provider request shape before accepting a replay as valid, including session metadata, message ordering, tool schema visibility, and continuation content after tool results. Controller/agent replay MUST additionally prove that shell-owned provider conversion does not drop user text, assistant tool-call context, tool-result content, or session metadata.

#### Scenario: controller request conversion preserves semantic content

- GIVEN a controller replay fixture runs through the shell-owned provider conversion path
- WHEN the fake provider records initial and continuation requests
- THEN the recorded requests SHALL include non-empty provider-native messages in the expected order
- AND the recorded requests SHALL preserve the expected session id or session metadata required by the fixture
- AND the continuation request SHALL include assistant tool-call context and tool-result content

### Requirement: Normalized replay output is byte-stable

The deterministic replay rail MUST normalize approved volatile fields and produce byte-stable replay artifacts for transcripts, event streams, provider requests, tool results, and receipts across both engine and controller replay rails.

#### Scenario: controller replay produces stable artifacts

- GIVEN the same deterministic controller replay fixture is executed twice in separate isolated temp state
- WHEN both runs complete successfully
- THEN their normalized transcript, event stream, provider request, tool result, and receipt artifacts SHALL be byte-identical
- AND their BLAKE3 hashes SHALL match
