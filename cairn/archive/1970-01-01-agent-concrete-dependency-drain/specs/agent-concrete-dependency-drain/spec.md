## ADDED Requirements

### Requirement: Agent reusable policy uses neutral ports [r[agent-concrete-dependency-drain.neutral-ports]]

Reusable agent turn, compaction, and tool-execution policy MUST depend on agent-owned ports and neutral DTOs rather than concrete provider, database, configuration, TUI, protocol, or router implementation types.

#### Scenario: selected dependency family moves behind a port [r[agent-concrete-dependency-drain.neutral-ports.selected-family]]
- GIVEN a concrete dependency family is selected for a drain slice
- WHEN reusable agent code needs that capability
- THEN the code MUST call a neutral agent port or DTO owner
- AND concrete composition MUST live at a named app-edge adapter

### Requirement: Dependency budget shrinks measurably [r[agent-concrete-dependency-drain.dependency-budget]]

Every agent dependency drain slice MUST update an inventory that names remaining concrete edges, their owners, and the next convergence condition.

#### Scenario: budget has fewer production edges [r[agent-concrete-dependency-drain.dependency-budget.decreases]]
- GIVEN the before-change inventory records production concrete imports
- WHEN the slice is complete
- THEN at least one production concrete edge MUST be removed or narrowed to an explicit adapter/test owner
- AND remaining edges MUST have owner receipts

### Requirement: Agent behavior remains stable [r[agent-concrete-dependency-drain.verification]]

Validation MUST prove the moved seam preserves turn, compaction, or tool execution behavior.

#### Scenario: focused tests cover the moved seam [r[agent-concrete-dependency-drain.verification.focused]]
- GIVEN a dependency edge moved behind a port
- WHEN focused agent tests run
- THEN the tests MUST exercise the neutral port and app-edge adapter path
