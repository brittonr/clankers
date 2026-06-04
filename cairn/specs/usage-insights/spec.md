# usage-insights Specification

## Purpose

This spec defines the canonical requirements for usage insights.

## Requirements

### Requirement: Aggregate usage report
The system SHALL produce a structured usage report from historical session data. The report SHALL include: total sessions, total tokens (input/output), estimated cost, tool call frequency by tool name, model distribution by token usage, and daily activity counts.

#### Scenario: Generate 30-day report
- GIVEN the scenario is evaluated
- **WHEN** the user runs `/insights` with no arguments
- **THEN** a report is generated for the last 30 days from the redb database

#### Scenario: Custom time window
- GIVEN the scenario is evaluated
- **WHEN** the user runs `/insights 7`
- **THEN** a report is generated for the last 7 days

#### Scenario: No data in time window
- GIVEN the scenario is evaluated
- **WHEN** no sessions exist within the requested time window
- **THEN** the system reports that no data is available for the period

---

### Requirement: Cost estimation
The system SHALL estimate costs using known per-model pricing data. For models with unknown pricing, the report SHALL display "unknown" rather than guessing.

#### Scenario: Known model pricing
- GIVEN the scenario is evaluated
- **WHEN** usage data includes Anthropic Claude models
- **THEN** costs are estimated using the published per-token rates

#### Scenario: Unknown model pricing
- GIVEN the scenario is evaluated
- **WHEN** usage data includes a custom or local model endpoint
- **THEN** the cost column shows "unknown" for that model

---

### Requirement: Terminal-friendly rendering
The report SHALL be rendered inline in the TUI conversation view using monospace-aligned tables and block-character bar charts. The report SHALL be readable without horizontal scrolling at 80 columns.

#### Scenario: Render in TUI
- GIVEN the scenario is evaluated
- **WHEN** the user runs `/insights` in the TUI
- **THEN** the report is displayed inline in the conversation view with formatted tables and bar charts

---

### Requirement: Tool usage breakdown
The report SHALL include a ranked list of tool calls by frequency, derived from the audit log.

#### Scenario: Tools ranked by frequency
- GIVEN the scenario is evaluated
- **WHEN** the report is generated
- **THEN** tools are listed in descending order of call count with counts shown
