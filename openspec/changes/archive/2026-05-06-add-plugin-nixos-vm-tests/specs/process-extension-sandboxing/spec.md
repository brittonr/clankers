## MODIFIED Requirements

### Requirement: Restricted sandbox mode fails closed
The host SHALL support an explicit `restricted` sandbox mode for stdio plugins. In restricted mode, the host SHALL enforce filtered environment, bounded writable roots, and network access only when both plugin permissions and sandbox policy allow it. If the host cannot apply the requested restrictions, it SHALL refuse to start the plugin. The `vm-plugin-runtime` NixOS VM check SHALL verify the enforced restricted boundary when the backend is available and SHALL otherwise verify fail-closed behavior.

#### Scenario: Restricted plugin denied startup when sandbox unavailable
- **WHEN** a stdio plugin requests `restricted` sandbox mode and the host cannot apply that mode on the current system
- **THEN** the plugin process is not launched
- **THEN** the plugin state becomes `error`

#### Scenario: Network denied by sandbox policy
- **WHEN** a stdio plugin does not have both logical network permission and sandbox network allowance
- **THEN** outbound network access is denied in restricted mode

#### Scenario: Writable roots are bounded
- **WHEN** a stdio plugin runs in restricted mode
- **THEN** writes are limited to its dedicated plugin state directory and any explicitly declared writable project roots
- **THEN** writes outside those roots are denied

#### Scenario: VM verifies sandbox boundary or fail-closed behavior [r[process-extension-sandboxing.restricted-mode.vm-boundary]]
- **WHEN** the `vm-plugin-runtime` NixOS VM check runs a restricted stdio fixture
- **THEN** the check proves allowed writes stay inside declared roots and denied writes/network attempts fail if the restricted backend is active
- **THEN** otherwise the check proves clankers refuses to start the restricted plugin rather than silently running it without restrictions
