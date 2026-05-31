# UCAN Concrete File Tool Paths Delta

## Purpose

Hardens the accepted UCAN + Basalt daemon auth tool-gate contract so public UCAN-gated file tools cannot authorize an omitted ambient default path.

## Requirements

### Requirement: Public file-tool invocations use concrete paths [r[ucan-basalt-daemon-auth.vocabulary.concrete-file-paths]]

Public UCAN + Basalt authorization MUST build file read/write invocation requests only from explicit, non-empty tool input paths.

#### Scenario: omitted file path fails closed [r[ucan-basalt-daemon-auth.vocabulary.concrete-file-paths.omitted]]
- GIVEN a public UCAN-gated read-only or write file tool call omits `path`
- WHEN Clankers maps the call into UCAN/Basalt admission requests
- THEN mapping MUST fail closed before the tool executes
- AND Clankers MUST NOT fabricate an ambient current-directory, project-root, wildcard, or default file resource

#### Scenario: blank file path fails closed [r[ucan-basalt-daemon-auth.vocabulary.concrete-file-paths.blank]]
- GIVEN a public UCAN-gated read-only or write file tool call provides an empty or whitespace-only `path`
- WHEN Clankers maps the call into UCAN/Basalt admission requests
- THEN mapping MUST fail closed before Basalt admission or tool execution
- AND the denial MUST use safe metadata without raw file contents, prompts, credentials, or token material

### Requirement: Public tool gate keeps concrete file authorization [r[ucan-basalt-daemon-auth.tool-gate.concrete-file-paths]]

The public UCAN + Basalt tool gate MUST require the generic `tool/use` authorization and the concrete file read/write authorization for file-oriented tools.

#### Scenario: concrete file path builds file request [r[ucan-basalt-daemon-auth.tool-gate.concrete-file-paths.present]]
- GIVEN a public UCAN-gated file tool call includes a non-empty `path`
- WHEN Clankers maps the call into admission requests
- THEN the request set MUST include `tool/use` for the tool
- AND it MUST include the matching concrete `file/read` or `file/write` request for the supplied path

### Requirement: Local legacy capability settings remain unchanged [r[ucan-basalt-daemon-auth.tool-gate.legacy-local-unchanged]]

This hardening MUST NOT change the legacy `settings.defaultCapabilities` tool-name filter used for local/non-public-UCAN sessions.

#### Scenario: legacy local gate keeps tool-name behavior [r[ucan-basalt-daemon-auth.tool-gate.legacy-local-unchanged.tool-only]]
- GIVEN a local legacy `UcanCapabilityGate` is built from `settings.defaultCapabilities`
- WHEN a tool-name-only capability authorizes a file-oriented tool without a path
- THEN this change MUST NOT add a new concrete-path requirement to that legacy local gate
- AND public UCAN + Basalt sessions MUST still use the stricter concrete-path requirement

### Requirement: Concrete file path verification is deterministic [r[ucan-basalt-daemon-auth.verification.concrete-file-paths]]

The implementation MUST include focused tests and a deterministic checker receipt for omitted-path denial, blank-path denial, concrete-path request construction, and local legacy gate non-regression.

#### Scenario: focused tests cover file path hardening [r[ucan-basalt-daemon-auth.verification.concrete-file-path-tests]]
- GIVEN focused tests run for the capability gate
- WHEN public UCAN-gated file tool calls omit, blank, or provide concrete paths
- THEN tests MUST prove omitted and blank paths deny before tool execution
- AND concrete paths still construct and authorize the expected file request

#### Scenario: checker writes redacted receipt [r[ucan-basalt-daemon-auth.verification.concrete-file-path-checker]]
- GIVEN implementation, tests, specs, and tasks are present
- WHEN the deterministic checker runs
- THEN it MUST write a receipt under `target/ucan-concrete-file-tool-paths/`
- AND the receipt MUST hash source artifacts without embedding raw compact UCAN tokens, signing keys, prompts, provider payloads, file contents, or tool input bodies

#### Scenario: closeout validation runs [r[ucan-basalt-daemon-auth.verification.concrete-file-path-closeout]]
- GIVEN implementation is complete
- WHEN the change is closed
- THEN focused Rust tests, checker receipt, Cairn validation/gates, sync/archive inspection, and diff checks MUST pass
