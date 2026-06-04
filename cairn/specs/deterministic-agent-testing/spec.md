# deterministic-agent-testing Specification

## Purpose

Define Clankers' credential-free deterministic agent-turn replay contract for scripted provider responses, scripted tool results, normalized replay artifacts, and BLAKE3-bound regression receipts.
## Requirements
### Requirement: Deterministic turn replay fixtures

Clankers MUST provide credential-free deterministic agent-turn replay fixtures that describe fixed user input, session metadata, scripted model responses, expected tool calls/results, and expected normalized outputs for replayable model/tool turns. The replay rail MUST include the pure engine reducer boundary, at least one controller/agent shell boundary, and at least one persisted-session resume boundary that restores prior history before building a provider request, all without live provider credentials, network access, daemon sockets, or ambient user config.

#### Scenario: persisted replay crosses session resume

- GIVEN a deterministic replay fixture that completes and persists an initial tool turn under isolated session state
- WHEN a fresh controller or agent resumes that session and processes a follow-up prompt
- THEN the resumed provider request SHALL include the restored user prompt, assistant tool-call context, tool-result context, follow-up prompt, and expected session metadata
- AND the replay SHALL complete without live credentials, network access, daemon sockets, or ambient user config

### Requirement: Provider request shape is pinned

The deterministic replay rail MUST assert provider request shape before accepting a replay as valid, including session metadata, message ordering, tool schema visibility, and continuation content after tool results. Controller/agent replay MUST prove shell-owned provider conversion does not drop user text, assistant tool-call context, tool-result content, or session metadata. Persisted-session replay MUST additionally prove restored history is present in the first provider request after resume.

#### Scenario: resumed request conversion preserves restored semantic content

- GIVEN a persisted deterministic replay fixture resumes a prior session
- WHEN the fake provider records the first request after resume
- THEN the recorded request SHALL include non-empty restored history before the new follow-up prompt
- AND it SHALL preserve session id metadata and restored tool-result context

### Requirement: Normalized replay output is byte-stable

The deterministic replay rail MUST normalize approved volatile fields and produce byte-stable replay artifacts for transcripts, event streams, provider requests, tool results, persistence/resume observations, and receipts across engine, controller, and persisted-session replay rails.

#### Scenario: persisted replay produces stable artifacts

- GIVEN the same persisted-session deterministic replay fixture is executed twice in separate isolated temp state
- WHEN both runs complete successfully
- THEN their normalized persisted history, resumed provider requests, event stream, tool results, and receipt artifacts SHALL be byte-identical
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
