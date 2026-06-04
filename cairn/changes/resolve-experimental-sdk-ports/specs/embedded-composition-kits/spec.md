## ADDED Requirements

### Requirement: Experimental SDK ports have an owner budget [r[embedded-composition-kits.experimental-port-budget]]

Every public embedded SDK item labeled `experimental` MUST have a recorded owner, use-site status, and disposition: promote with evidence, keep experimental with rationale, or make private.

#### Scenario: experimental inventory is actionable [r[embedded-composition-kits.experimental-port-budget.actionable]]
- GIVEN the generated SDK inventory contains experimental rows
- WHEN the experimental budget rail runs
- THEN each row MUST be grouped by crate and owner module
- AND each group MUST name the next convergence action and validation path

#### Scenario: unused experimental ports do not remain public by accident [r[embedded-composition-kits.experimental-port-budget.hide-unused]]
- GIVEN an experimental port has no production adapter, fixture, or documented product recipe use
- WHEN the port is reviewed during a resolution slice
- THEN it MUST either gain deterministic evidence or become private/compatibility-scoped
