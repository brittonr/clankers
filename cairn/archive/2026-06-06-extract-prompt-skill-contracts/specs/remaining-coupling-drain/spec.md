## MODIFIED Requirements

### Requirement: Runtime defaults fail closed without ambient services [r[remaining-coupling-drain.runtime-fail-closed-defaults]]

Runtime facade services that require provider, auth, plugin, process, prompt filesystem, skill, session, or storage behavior MUST fail closed unless a host explicitly injects the required service.

#### Scenario: prompt and skill contracts are host injected [r[remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection]]
- GIVEN prompt assembly or skill lookup needs filesystem/config/project state
- WHEN an embedded host uses runtime defaults without injecting prompt or skill services
- THEN runtime MUST return typed unavailable diagnostics instead of reading `.clankers`, `.pi`, global config, or project skill directories
- AND reusable prompt/skill DTOs MUST live in a neutral owner independent of desktop path discovery
