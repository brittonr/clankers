## ADDED Requirements

### Requirement: Plugin Runtime Extension Execution [r[tool-host-embedding.plugin-runtime-execution]]

The system MUST support an explicitly injected plugin runtime extension service that can publish plugin tool descriptors and execute a plugin tool without letting the embedded runtime discover plugin roots or launch extension runtimes implicitly.

#### Scenario: Desktop host injects plugin runtime [r[tool-host-embedding.plugin-runtime-execution.desktop-injected]]

- GIVEN a desktop host supplies a plugin manager to the runtime service adapter
- WHEN the host asks the extension runtime service for plugin publishable tools
- THEN descriptors are derived from that injected manager without using hidden runtime discovery

#### Scenario: Plugin tool execution returns safe receipt [r[tool-host-embedding.plugin-runtime-execution.safe-receipt]]

- GIVEN a desktop host supplies a plugin manager containing a loaded plugin tool
- WHEN the host executes that plugin tool through the extension runtime service
- THEN the plugin is invoked through the injected manager and the returned receipt records status and safe identifiers without raw plugin arguments, raw plugin output, credentials, headers, or environment values
