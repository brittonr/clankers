## ADDED Requirements

### Requirement: TUI experiment dashboard widget
The system SHALL provide a TUI widget that displays the current autoresearch session status. The widget MUST show: session name, primary metric name and direction, current best value, total runs, run breakdown by status (kept/discarded/crashed), and a table of recent results (run number, commit, metric, status, description).

#### Scenario: Dashboard with active session
- **WHEN** the user toggles the experiment dashboard (Ctrl+X or leader menu)
- **AND** `autoresearch.jsonl` exists with a config line and result lines
- **THEN** the dashboard renders showing session name, best metric, run count, status breakdown, and a scrollable results table

#### Scenario: Dashboard with no session
- **WHEN** the user toggles the experiment dashboard
- **AND** no `autoresearch.jsonl` exists
- **THEN** the dashboard shows "No active experiment session"

#### Scenario: Dashboard updates after each log
- **WHEN** `log_experiment` completes
- **THEN** the dashboard widget refreshes to show the new result

### Requirement: Dashboard keybinding
The system SHALL toggle the experiment dashboard via Ctrl+X keybinding in the TUI. The keybinding MUST work in the main input mode (not during leader menu or other modal states).

#### Scenario: Toggle on
- **WHEN** the user presses Ctrl+X and the dashboard is not visible
- **THEN** the dashboard panel appears

#### Scenario: Toggle off
- **WHEN** the user presses Ctrl+X and the dashboard is visible
- **THEN** the dashboard panel hides

### Requirement: Leader menu entry
The system SHALL add an "Experiments" entry to the leader menu that toggles the experiment dashboard, as an alternative to the Ctrl+X keybinding.

#### Scenario: Leader menu activation
- **WHEN** the user opens the leader menu and selects the experiments entry
- **THEN** the experiment dashboard toggles visibility
