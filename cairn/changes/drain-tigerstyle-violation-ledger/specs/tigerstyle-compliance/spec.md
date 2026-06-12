## ADDED Requirements

### Requirement: Violation ledger names every burn-down allow site [r[tigerstyle-compliance.violation-ledger]]

The project SHALL maintain a lifecycle task ledger for every source Tigerstyle allow site that is part of the current burn-down scope. The ledger MUST identify the source path, the lint names allowed at that site, and the implementation task that owns draining or narrowing the site.

#### Scenario: Inventory names each allow site [r[tigerstyle-compliance.violation-ledger.inventory]]
- GIVEN a source file contains a `tigerstyle::...` allow in the burn-down scope
- WHEN the Tigerstyle compliance change package is inspected
- THEN the change design or tasks ledger SHALL identify that source path and the relevant lint names

### Requirement: Slice validation evidence is checked in [r[tigerstyle-compliance.slice-validation]]

Every completed Tigerstyle drain slice SHALL have checked-in validation evidence. The evidence MUST name the exact commands run, the exit status for each command, and enough output to show the focused package validation and full Tigerstyle audit result.

#### Scenario: Verification task is completed [r[tigerstyle-compliance.slice-validation.completed-task]]
- GIVEN a Tigerstyle drain verification task is checked off
- WHEN its evidence path is read
- THEN the evidence SHALL include the command list, exit statuses, and full Tigerstyle audit result for the slice

### Requirement: Boundary exceptions are narrow and local [r[tigerstyle-compliance.boundary-exceptions]]

A Tigerstyle allow MAY remain only when it is a narrow documented boundary exception. Remaining exceptions MUST be local rather than broad crate-level burn-down allows, and their evidence MUST explain why the exception is narrower than the lint risk.

#### Scenario: Local allow remains after review [r[tigerstyle-compliance.boundary-exceptions.local-review]]
- GIVEN a local Tigerstyle allow remains after its implementation task completes
- WHEN the verification evidence for that task is inspected
- THEN the evidence SHALL explain the boundary
- AND the evidence SHALL show a successful full Tigerstyle audit

### Requirement: Public API movement receives root validation [r[tigerstyle-compliance.public-api-validation]]

Any Tigerstyle drain slice that changes a public API SHALL run a root compile validation in addition to package-focused tests. The root compile validation MUST be recorded in the slice evidence.

#### Scenario: Public utility API changes [r[tigerstyle-compliance.public-api-validation.root-compile]]
- GIVEN a drain slice changes a public function signature or public type
- WHEN the slice verification task is checked off
- THEN its evidence SHALL include `cargo test -p clankers --no-run` or a stricter root/workspace validation command
