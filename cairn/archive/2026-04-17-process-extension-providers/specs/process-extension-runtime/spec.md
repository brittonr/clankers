## ADDED Requirements

### Requirement: Process-backed plugin manifests
The system SHALL support plugin manifests with `kind: "stdio"` in the same global and project plugin directories as existing Extism plugins. A stdio plugin manifest SHALL declare the launch command, optional arguments, and optional launch metadata without requiring a WASM module.

#### Scenario: Mixed plugin kinds discovered
- **WHEN** the host discovers one Extism plugin and one `kind: "stdio"` plugin in the configured plugin directories
- **THEN** both plugins appear in the discovered plugin set
- **THEN** the Extism plugin is loaded through the existing WASM path and the stdio plugin is prepared for process startup

#### Scenario: Invalid stdio manifest rejected
- **WHEN** a `kind: "stdio"` plugin manifest omits its required launch command
- **THEN** the plugin is marked `error`
- **THEN** all other valid plugins continue loading normally

---

### Requirement: Supervised stdio plugin lifecycle
Enabled stdio plugins SHALL be launched during plugin initialization in both standalone and daemon modes, SHALL complete a ready handshake before entering the `active` state, and SHALL be restarted after unexpected exit using the fixed backoff sequence `1s`, `2s`, `4s`, `8s`, `16s`. After 5 consecutive failed startups or crash loops without a successful ready state, the plugin SHALL enter `error`. Manual disable and normal host shutdown SHALL stop the plugin without scheduling a restart.

#### Scenario: Plugin becomes active after ready handshake
- **WHEN** an enabled stdio plugin process starts and completes the ready handshake successfully
- **THEN** the host marks the plugin `active`
- **THEN** the plugin can register tools and event subscriptions

#### Scenario: Standalone mode launches stdio plugin during initialization
- **WHEN** clankers starts in standalone interactive mode with an enabled stdio plugin
- **THEN** the plugin is launched during plugin initialization
- **THEN** tools it registers after `ready` are included in standalone tool construction

#### Scenario: Unexpected exit enters backoff and restarts
- **WHEN** an active stdio plugin exits unexpectedly
- **THEN** the host marks the plugin `backoff`
- **THEN** the host retries startup after `1s`, then `2s`, then `4s`, then `8s`, then `16s` until the plugin becomes ready again or reaches the failure limit

#### Scenario: Crash loop enters error state
- **WHEN** a stdio plugin fails 5 consecutive startup attempts without reaching a successful ready state
- **THEN** the host marks the plugin `error`
- **THEN** the plugin is not exposed as active until the user reloads plugins or restarts the host

#### Scenario: Disabled plugin is not launched
- **WHEN** a stdio plugin is disabled before initialization or via the existing disable flow
- **THEN** the host does not launch the plugin process
- **THEN** the plugin state remains `disabled`
