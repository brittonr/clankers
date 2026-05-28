# Config Prompt Skill Services Specification

## Purpose

Defines the `config-prompt-skill-services` capability.

## Requirements

### Requirement: Configuration core is display-neutral [r[config-prompt-skill-services.config-core]]

Reusable configuration/path settings MUST NOT require terminal display, keybinding widget, router daemon, UCAN runtime, or TUI theme types unless those concerns live behind explicit desktop/display adapters.

#### Scenario: Config DTOs avoid TUI/ratatui leakage [r[config-prompt-skill-services.config-core.display-neutral]]
- GIVEN reusable config modules are inspected
- WHEN they define settings, paths, and embedded runtime defaults
- THEN they MUST use plain data or neutral DTOs
- AND TUI themes, `ratatui::Color`, keymap widgets, router daemon handles, and shell authorization runtime types MUST be absent unless feature-gated or adapter-owned

### Requirement: Prompt assembly is host-owned [r[config-prompt-skill-services.prompt-service]]

Prompt assembly MUST be expressed through host-provided services and safe defaults rather than implicit filesystem or shell-global prompt discovery in generic runtime paths.

#### Scenario: Host-owned prompt sources are explicit [r[config-prompt-skill-services.prompt-service.host-owned]]
- GIVEN an embedded host assembles a prompt
- WHEN it provides system text, host context, filesystem context, context references, or skill snippets
- THEN each source MUST be explicit in service inputs or policy
- AND disabled or absent sources MUST produce safe unsupported metadata or fail closed without probing dotdirs or project files implicitly

### Requirement: Skill resolution is explicit [r[config-prompt-skill-services.skill-service]]

Skill loading MUST be owned by host or desktop skill services and MUST NOT be required by the generic SDK path.

#### Scenario: Skill roots are adapter-owned [r[config-prompt-skill-services.skill-service.explicit-roots]]
- GIVEN skill content is used in a prompt
- WHEN runtime or agent code requests skills
- THEN it MUST call a skill resolver service with explicit roots or already-loaded content
- AND absent skill services MUST not cause global/project skill directory discovery in embedded mode

### Requirement: Desktop adapters preserve current behavior [r[config-prompt-skill-services.desktop-adapter]]

CLI, TUI, and daemon shells MAY keep current settings/theme/keybinding/prompt/skill behavior only through explicit desktop adapters.

#### Scenario: Desktop behavior is adapter-selected [r[config-prompt-skill-services.desktop-adapter.parity]]
- GIVEN normal Clankers shells start
- WHEN they load settings, prompt sources, themes, keybindings, and skills
- THEN they MUST select desktop adapters explicitly
- AND focused parity tests MUST show existing desktop behavior is preserved after projection

### Requirement: Config/prompt/skill verification is deterministic [r[config-prompt-skill-services.verification]]

Verification MUST combine source-boundary rails and prompt/skill behavioral fixtures.

#### Scenario: Config rail catches display leakage [r[config-prompt-skill-services.verification.config-rail]]
- GIVEN reusable config-core modules are checked
- WHEN forbidden display/router/runtime imports appear
- THEN the rail MUST fail with the offending module and owner requirement

#### Scenario: Prompt and skill fixtures cover defaults [r[config-prompt-skill-services.verification.prompt-skill-fixtures]]
- GIVEN prompt/skill fixtures run
- WHEN host-only, filesystem-disabled, desktop-enabled, missing-skill-service, and redaction cases execute
- THEN each MUST assert assembled prompt content, provenance, safe metadata, and fail-closed behavior

#### Scenario: Closeout validates service split [r[config-prompt-skill-services.verification.closeout]]
- GIVEN implementation is complete
- WHEN focused validation runs
- THEN config/runtime/prompt/skill tests, SDK checks if public docs changed, Cairn validation/gates, and diff checks MUST pass
