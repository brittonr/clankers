## MODIFIED Requirements

### Requirement: Experimental SDK ports have an owner budget [r[embedded-composition-kits.experimental-port-budget]]

Every public embedded SDK item labeled `experimental` MUST have a recorded owner, use-site status, and disposition: promote with evidence, keep experimental with rationale, or make private.

#### Scenario: engine buffered tool results are promoted with reducer evidence [r[embedded-composition-kits.experimental-port-budget.engine-buffered-results-supported]]
- GIVEN `EngineState` exposes buffered tool feedback while waiting for multiple tool calls
- WHEN the engine buffered reducer tests and experimental budget rail run
- THEN `EngineBufferedToolResult` and its public fields MUST be classified as supported or hidden, not left experimental
- AND the experimental budget count MUST decrease only when the generated inventory and policy agree on the new stability
