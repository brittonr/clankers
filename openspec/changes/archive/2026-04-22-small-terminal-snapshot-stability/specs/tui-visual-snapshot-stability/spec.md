## ADDED Requirements

### Requirement: Small-terminal startup structure snapshot is deterministic
The system SHALL produce a deterministic startup structure snapshot for the 12x50 TUI visual test so `tests/tui/visual.rs::snapshot_small_terminal` does not fail from unexplained drift in normal test execution.

#### Scenario: Focused small-terminal snapshot run passes
- **WHEN** the small-terminal visual test is run in isolation
- **THEN** the rendered startup structure matches the checked-in `small_12x50_structure` snapshot
- **AND** the test does not require ad hoc snapshot regeneration to pass

#### Scenario: Broader test execution preserves the same startup structure
- **WHEN** the same small-terminal visual test is reached from a broader automated test run
- **THEN** it observes the same startup structure baseline as the focused run
- **AND** it does not fail because suite ordering or startup timing changed the captured frame

### Requirement: Small-terminal snapshot covers stable startup layout only
The system SHALL ensure the 12x50 startup snapshot asserts stable layout structure rather than transient or unrelated startup content.

#### Scenario: Stable startup frame is captured before assertion
- **WHEN** the visual harness captures the 12x50 startup structure for assertion
- **THEN** it captures a settled startup frame intended by the test
- **AND** the asserted structure excludes volatile content that is not part of the intended small-terminal layout contract

#### Scenario: Intentional layout changes update the baseline explicitly
- **WHEN** the investigation proves the current 12x50 startup layout is correct but the checked-in snapshot is stale
- **THEN** the checked-in snapshot is updated to the new deterministic structure
- **AND** the change records enough verification to show the new baseline was accepted intentionally rather than by accident
