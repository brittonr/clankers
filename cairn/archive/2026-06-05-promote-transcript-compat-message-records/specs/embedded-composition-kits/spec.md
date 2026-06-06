## MODIFIED Requirements

### Requirement: Experimental SDK ports have an owner budget [r[embedded-composition-kits.experimental-port-budget]]

Every public embedded SDK item labeled `experimental` MUST have a recorded owner, use-site status, and disposition: promote with evidence, keep experimental with rationale, or make private.

#### Scenario: transcript compatibility records are optional supported [r[embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported]]
- GIVEN `clanker-message` keeps Clankers transcript records behind the non-default `transcript-compat` feature
- WHEN the transcript compatibility tests, message-contract boundary rail, and experimental budget rail run
- THEN `UserMessage`, `AssistantMessage`, `ToolResultMessage`, and their public fields MUST be classified as supported optional compatibility API or hidden, not left experimental
- AND the experimental budget count MUST reach zero only when the generated inventory and policy agree on the new stability
