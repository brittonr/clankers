# plugin-nixos-vm-tests Specification

## Purpose

Defines the NixOS VM runtime coverage required for packaged Clankers plugins, including packaged binary boot, Extism plugin discovery and invocation, stdio fixture lifecycle, restricted sandbox behavior, and harness integration.

## Requirements
### Requirement: Plugin runtime NixOS VM check [r[plugin-nixos-vm-tests.vm-check]]
The repository MUST expose a flake check named `vm-plugin-runtime` that boots a NixOS VM with the packaged clankers binary and verifies deterministic plugin runtime behavior from the installed environment.

#### Scenario: VM check is exported [r[plugin-nixos-vm-tests.vm-check.exported]]
- GIVEN the current system supports NixOS VM checks
- WHEN a user evaluates `.#checks.$system.vm-plugin-runtime.name`
- THEN the check exists and is named for plugin runtime coverage

#### Scenario: VM boots with packaged clankers [r[plugin-nixos-vm-tests.vm-check.boots-packaged-clankers]]
- GIVEN the `vm-plugin-runtime` check is built
- WHEN the VM reaches `default.target`
- THEN the installed `clankers` binary is available inside the VM
- AND plugin roots required by the test are present inside the VM

### Requirement: Packaged Extism plugin invocation in VM [r[plugin-nixos-vm-tests.extism-invocation]]
The VM check MUST invoke at least one safe shipped Extism plugin tool through the installed clankers plugin/tool surface and assert a deterministic result.

#### Scenario: Safe Extism tool returns expected output [r[plugin-nixos-vm-tests.extism-invocation.safe-tool]]
- GIVEN the VM has the packaged shipped plugins available
- WHEN the test invokes a deterministic Extism plugin tool such as hash or text-stats
- THEN the invocation succeeds
- AND the output matches the expected deterministic result

#### Scenario: Packaged manifest and WASM layout are discoverable [r[plugin-nixos-vm-tests.extism-invocation.packaged-layout]]
- GIVEN the VM uses plugin artifacts from the Nix-built package layout
- WHEN clankers discovers plugins inside the VM
- THEN the selected Extism plugin manifest is found
- AND its WASM module is loadable from the packaged location

### Requirement: Stdio plugin lifecycle in VM [r[plugin-nixos-vm-tests.stdio-lifecycle]]
The VM check MUST stage the reference stdio plugin fixture and verify launch, handshake, live tool registration, invocation, and clean shutdown or disable behavior through the installed host.

#### Scenario: Stdio fixture registers a live tool [r[plugin-nixos-vm-tests.stdio-lifecycle.registers-tool]]
- GIVEN the reference stdio echo fixture is installed into a scanned plugin root inside the VM
- WHEN clankers initializes plugins in a runtime-capable mode
- THEN the stdio plugin process completes hello/ready startup
- AND its registered tool appears in the live plugin tool inventory

#### Scenario: Stdio fixture invocation succeeds [r[plugin-nixos-vm-tests.stdio-lifecycle.invokes-tool]]
- GIVEN the stdio fixture has registered its echo tool
- WHEN the VM test invokes that tool with deterministic input
- THEN the plugin receives the invocation over the framed stdio protocol
- AND clankers returns the expected deterministic echo result

#### Scenario: Stdio fixture stops cleanly [r[plugin-nixos-vm-tests.stdio-lifecycle.stops-cleanly]]
- GIVEN the stdio fixture process is active in the VM
- WHEN clankers shuts down the plugin host or disables the plugin
- THEN the host sends the shutdown path or stops supervision intentionally
- AND no restart/backoff loop is scheduled for that normal stop

### Requirement: Restricted sandbox VM boundary [r[plugin-nixos-vm-tests.restricted-sandbox]]
When restricted stdio sandbox support is available in the NixOS VM, the check MUST prove bounded write/network behavior; when unavailable, the check MUST prove fail-closed startup for restricted stdio plugins.

#### Scenario: Restricted sandbox enforces bounded behavior [r[plugin-nixos-vm-tests.restricted-sandbox.enforced]]
- GIVEN the VM host/backend can apply restricted stdio sandboxing
- WHEN a restricted stdio fixture attempts allowed and denied filesystem or network actions
- THEN allowed actions inside declared roots succeed
- AND denied actions outside declared roots or without network allowance fail

#### Scenario: Restricted sandbox fails closed when unavailable [r[plugin-nixos-vm-tests.restricted-sandbox.fail-closed]]
- GIVEN the VM cannot apply the requested restricted sandbox backend
- WHEN a stdio plugin requests `sandbox: "restricted"`
- THEN clankers refuses to start the plugin
- AND the plugin status reports an error rather than running unrestricted

### Requirement: VM harness integration [r[plugin-nixos-vm-tests.harness]]
The canonical test harness MUST support running the plugin VM check explicitly and SHOULD include it in the broad VM selector once it is stable enough for the local release-readiness rail.

#### Scenario: Explicit VM selector runs plugin runtime check [r[plugin-nixos-vm-tests.harness.explicit-selector]]
- GIVEN the implementation adds `vm-plugin-runtime`
- WHEN a user runs `./scripts/test-harness.sh vm vm-plugin-runtime`
- THEN the harness builds `.#checks.$system.vm-plugin-runtime`
- AND the summary records the plugin VM step pass/fail result

#### Scenario: Broad VM selector includes stable plugin check [r[plugin-nixos-vm-tests.harness.broad-selector]]
- GIVEN the plugin VM check is deterministic and credential-free
- WHEN a user runs `./scripts/test-harness.sh vm all`
- THEN the plugin VM check is included unless explicitly documented as too expensive or host-dependent
