## ADDED Requirements

### Requirement: Embedded SDK verification hygiene [r[embeddable-agent-engine.verification-hygiene]]
The system MUST keep embedded SDK acceptance commands reproducible from ordinary shell environments and keep verification bookkeeping accurate enough for future drain automation.

#### Scenario: acceptance script ignores caller CDPATH [r[embeddable-agent-engine.verification-hygiene.cdpath]]
- GIVEN a caller shell has `CDPATH` set to a directory that causes `cd` to print paths
- WHEN the embedded SDK acceptance script computes its script directory and repository root
- THEN the computed paths remain valid repository paths
- THEN the script does not require callers to prefix the command with `CDPATH=`

#### Scenario: focused verification remains warning-clean for touched adapter helpers [r[embeddable-agent-engine.verification-hygiene.warning-clean]]
- GIVEN a change touches agent turn adapter helpers as part of decoupling cleanup
- WHEN focused agent turn tests are run
- THEN obsolete helpers are removed or locally justified
- THEN the verification output does not introduce new dead-code warnings for the touched helpers

#### Scenario: drain-state reflects idle queue state [r[embeddable-agent-engine.verification-hygiene.drain-state]]
- GIVEN `openspec list` reports no active changes after a queue drain
- WHEN drain-state bookkeeping is inspected
- THEN it reports no current active change and does not claim a pending last commit
- THEN future drain reviews can treat the file as advisory idle state rather than stale implementation state
