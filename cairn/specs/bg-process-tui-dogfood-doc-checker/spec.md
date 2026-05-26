# BG-process TUI Dogfood Docs Checker Specification

## Purpose

Defines the `bg-process-tui-dogfood-doc-checker` capability: deterministic docs contract coverage that keeps BG-process TUI dogfood command spelling and receipt criteria aligned with the maintained harness rail without replacing the live tmux/TUI dogfood proof.

## Requirements

### Requirement: Docs [r[bg-process-tui-dogfood-doc-checker.command-drift]]

Docs MUST keep the BG-process TUI dogfood command spelling aligned with the harness selector.

#### Scenario: Command spelling stays aligned
- GIVEN the docs mention BG-process TUI dogfood
- WHEN the checker runs
- THEN it verifies `./scripts/test-harness.sh dogfood bg-process-tui` or the canonical equivalent appears

### Requirement: Docs [r[bg-process-tui-dogfood-doc-checker.receipt-criteria]]

Docs MUST include the required receipt criteria for operator review.

#### Scenario: Receipt criteria stay documented
- GIVEN the dogfood receipt schema is documented
- WHEN the checker runs
- THEN it fails if active-process, command-visible, layout-toggle-visible, or cleanup criteria are omitted

### Requirement: checker [r[bg-process-tui-dogfood-doc-checker.negative-fixture]]

The checker MUST include a negative fixture or test proving omissions fail.

#### Scenario: Omitted field is caught
- GIVEN a fixture omits a required dogfood receipt criterion
- WHEN the checker test runs
- THEN it reports a deterministic failure for the missing criterion

### Requirement: docs checker [r[bg-process-tui-dogfood-doc-checker.runtime-boundary]]

The docs checker MUST remain deterministic and not replace the live dogfood rail.

#### Scenario: Checker is deterministic
- GIVEN the docs checker runs in a normal test
- WHEN it validates documentation
- THEN it does not start tmux or a live model and points to the live dogfood rail for runtime proof
