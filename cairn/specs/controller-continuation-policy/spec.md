# controller-continuation-policy Specification

## Requirements

### Requirement: Controller continuation kit proves post-prompt state transitions

The controller-continuation-policy-kit SHALL make post-prompt actions deterministic and fail closed on stale effects.

#### Scenario: queued user prompt
- GIVEN a queued user prompt and loop continuation are both possible
- WHEN post-prompt policy resolves the next action
- THEN queued user prompt replay MUST take priority.

#### Scenario: stale effect ids fail closed
- GIVEN a stale, duplicate, or mismatched follow-up effect id is observed
- WHEN continuation policy evaluates it
- THEN state MUST remain unchanged and no stale follow-up may run.
