## ADDED Requirements

### Requirement: Turn Loop Decomposition [r[turn-loop.decomposition]]

The agent turn loop MUST be decomposed into explicit functional-core state/policy modules and thin imperative host adapters without changing observed turn behavior.

#### Scenario: Behavior parity [r[turn-loop.decomposition.scenario.1]]

- GIVEN an existing prompt/tool/model-switch regression fixture
- WHEN the decomposed turn loop runs the fixture
- THEN the transcript, emitted events, tool outcomes, cancellation behavior, and usage observations match the pre-decomposition baseline

#### Scenario: Boundary review [r[turn-loop.decomposition.scenario.2]]

- GIVEN a future change touches model streaming or tool execution
- WHEN the changed code is reviewed
- THEN the code path is isolated behind a named module or trait with focused tests rather than requiring edits to the monolithic turn loop
