# process-extension-sandboxing Specification

## Purpose

Defines stdio plugin launch-policy and sandboxing behavior, including explicit environment forwarding, working-directory selection, restricted sandbox enforcement, and fail-closed behavior when restrictions cannot be applied.
## Requirements
### Requirement: Explicit launch policy for stdio plugins

A stdio plugin manifest SHALL declare its launch policy: command, arguments, sandbox mode, optional working-directory mode, and environment allowlist. In this first change, every allowlisted environment variable is a required launch input. The host SHALL use that policy when starting the plugin process. In v1, the host-required runtime environment-variable exception set SHALL be empty: the host resolves `stdio.command` before spawn instead of relying on child `PATH` lookup, so no implicit environment variables are forwarded beyond the manifest allowlist.

#### Scenario: Allowlisted environment forwarded
- GIVEN the scenario is evaluated

- **WHEN** a stdio plugin manifest allowlists `GITHUB_TOKEN` and `FASTMAIL_TOKEN`
- **THEN** the launched plugin process receives only those declared variables
- **THEN** unrelated host environment variables are not inherited implicitly

#### Scenario: Missing allowlisted environment variable blocks startup
- GIVEN the scenario is evaluated

- **WHEN** a stdio plugin manifest allowlists `GITHUB_TOKEN` and that variable is absent from the host environment at launch time
- **THEN** the plugin process is not launched
- **THEN** the plugin state becomes `error` with a message naming the missing variable

#### Scenario: Working directory selected from policy
- GIVEN the scenario is evaluated

- **WHEN** a stdio plugin manifest requests project-root working directory mode
- **THEN** the launched plugin process starts with the project root as its working directory

### Requirement: Restricted sandbox mode fails closed
The host SHALL support an explicit `restricted` sandbox mode for stdio plugins. In restricted mode, the host SHALL enforce filtered environment, bounded writable roots, and network access only when both plugin permissions and sandbox policy allow it. If the host cannot apply the requested restrictions, it SHALL refuse to start the plugin. The `vm-plugin-runtime` NixOS VM check SHALL verify the enforced restricted boundary when the backend is available and SHALL otherwise verify fail-closed behavior.

#### Scenario: Restricted plugin denied startup when sandbox unavailable
- GIVEN the scenario is evaluated
- **WHEN** a stdio plugin requests `restricted` sandbox mode and the host cannot apply that mode on the current system
- **THEN** the plugin process is not launched
- **THEN** the plugin state becomes `error`

#### Scenario: Network denied by sandbox policy
- GIVEN the scenario is evaluated
- **WHEN** a stdio plugin does not have both logical network permission and sandbox network allowance
- **THEN** outbound network access is denied in restricted mode

#### Scenario: Writable roots are bounded
- GIVEN the scenario is evaluated
- **WHEN** a stdio plugin runs in restricted mode
- **THEN** writes are limited to its dedicated plugin state directory and any explicitly declared writable project roots
- **THEN** writes outside those roots are denied

#### Scenario: VM verifies sandbox boundary or fail-closed behavior [r[process-extension-sandboxing.restricted-mode.vm-boundary]]
- GIVEN the scenario is evaluated
- **WHEN** the `vm-plugin-runtime` NixOS VM check runs a restricted stdio fixture
- **THEN** the check proves allowed writes stay inside declared roots and denied writes/network attempts fail if the restricted backend is active
- **THEN** otherwise the check proves clankers refuses to start the restricted plugin rather than silently running it without restrictions

### Requirement: Inherit mode is explicit

The host SHALL also support an explicit `inherit` sandbox mode for stdio plugins. In `inherit` mode, the plugin runs with normal child-process privileges for the current clankers process, but still uses the manifest's command, argument, working-directory, and environment allowlist policy.

#### Scenario: Inherit mode launches without restricted backend
- GIVEN the scenario is evaluated

- **WHEN** a stdio plugin selects `inherit` sandbox mode
- **THEN** the host may launch it without applying the restricted sandbox backend
- **THEN** environment filtering and declared working directory still apply
