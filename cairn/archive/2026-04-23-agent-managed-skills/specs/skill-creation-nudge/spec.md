## ADDED Requirements

### Requirement: Periodic skill creation nudge
The system SHALL track tool-calling iterations during an agent session. After a configurable number of consecutive tool-calling turns (default 15), the system SHALL inject a reminder that the agent can create skills from successful approaches.

#### Scenario: Nudge after sustained tool use
- **WHEN** the agent has executed 15 consecutive tool-calling turns without invoking `skill_manage`
- **THEN** a system message is injected reminding the agent it can create skills

#### Scenario: Counter resets on skill_manage
- **WHEN** the agent calls `skill_manage`
- **THEN** the tool-calling turn counter resets to zero

#### Scenario: Nudge disabled
- **WHEN** the skill creation nudge interval is set to 0 in settings
- **THEN** no nudge messages are injected

---

### Requirement: Configurable nudge interval
The nudge interval SHALL be configurable via `settings.skills.creation_nudge_interval`. A value of 0 SHALL disable nudging entirely.

#### Scenario: Custom interval
- **WHEN** `creation_nudge_interval` is set to 25
- **THEN** the nudge fires after 25 consecutive tool-calling turns
