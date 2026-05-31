# UCAN Canonical File Tool Paths Delta

## Purpose

Hardens public UCAN + Basalt file-tool authorization so relative file paths resolve to deterministic session-rooted resources and parent traversal escapes deny before tool execution.

## Requirements

### Requirement: Public file-tool paths are canonical authorization resources [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths]]

Public UCAN + Basalt authorization MUST normalize file-tool paths into deterministic `clankers:file:...` resources before building `file/read` or `file/write` requests.

#### Scenario: Public auth carries session file root [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.root-context]]
- GIVEN a remote public UCAN-authenticated session is created, attached, or recovered
- WHEN Clankers constructs the public tool authorization context
- THEN it MUST include the session/project file root used to resolve relative file-tool paths
- AND absence of a required root for relative path resolution MUST deny before Basalt admission or tool execution

#### Scenario: Relative paths resolve under the session root [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.relative-resolution]]
- GIVEN a public UCAN-gated file tool call includes a relative path such as `src/lib.rs`
- AND the public authorization context has file root `/workspace/project`
- WHEN Clankers maps the call into admission requests
- THEN the file request resource MUST be `clankers:file:/workspace/project/src/lib.rs`
- AND the request MUST use the matching `file/read` or `file/write` ability

#### Scenario: Parent traversal escapes deny [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.traversal-denial]]
- GIVEN a public UCAN-gated file tool call includes `..` traversal that would escape the session file root
- WHEN Clankers maps the call into admission requests
- THEN mapping MUST fail closed before Basalt admission or tool execution
- AND Clankers MUST NOT rewrite the traversal into an ambient, project-parent, wildcard, or default resource

#### Scenario: Absolute paths keep explicit resource semantics [r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.absolute-paths]]
- GIVEN a public UCAN-gated file tool call includes an absolute path
- WHEN Clankers maps the call into admission requests
- THEN the absolute path MUST remain the basis for the concrete `clankers:file:...` resource
- AND UCAN/Basalt grants and policy MUST decide whether that absolute resource is allowed

### Requirement: Public tool gate uses canonical file paths [r[ucan-basalt-daemon-auth.tool-gate.canonical-file-tool-paths]]

The public UCAN + Basalt tool gate MUST use canonical file authorization resources for every file read/write request it adds to a tool invocation.

#### Scenario: Canonical path request is evaluated at call time [r[ucan-basalt-daemon-auth.tool-gate.canonical-file-tool-paths.call-time]]
- GIVEN a public UCAN-authenticated session attempts a read-only or write file tool call
- WHEN the tool call is authorized
- THEN Clankers MUST authorize generic `tool/use` and the canonical `file/read` or `file/write` request at call time
- AND the tool MUST execute only when all required canonical requests are allowed

#### Scenario: Legacy local gate remains unchanged [r[ucan-basalt-daemon-auth.tool-gate.legacy-local-unchanged.canonical-file-paths]]
- GIVEN a local legacy `UcanCapabilityGate` is built from `settings.defaultCapabilities`
- WHEN it authorizes a file-oriented tool call
- THEN this change MUST NOT add session-root canonicalization requirements to that legacy local gate
- AND public UCAN + Basalt sessions MUST still use canonical file-path request construction

### Requirement: Canonical file path verification is deterministic [r[ucan-basalt-daemon-auth.verification.canonical-file-tool-paths]]

The implementation MUST include focused tests and a deterministic checker receipt for session-root threading, relative path resolution, traversal denial, absolute-path semantics, public call-time authorization, and legacy local gate non-regression.

#### Scenario: Focused tests cover canonical file paths [r[ucan-basalt-daemon-auth.verification.canonical-file-path-tests]]
- GIVEN focused capability-gate tests run
- WHEN public UCAN-gated file tools use relative, traversal, and absolute paths
- THEN tests MUST prove relative paths resolve under the session root
- AND traversal escapes deny before Basalt admission or tool execution
- AND absolute paths keep explicit resource semantics
- AND the legacy local gate remains unchanged

#### Scenario: Checker writes redacted canonicalization receipt [r[ucan-basalt-daemon-auth.verification.canonical-file-path-checker]]
- GIVEN implementation, tests, specs, and tasks are present
- WHEN the deterministic checker runs
- THEN it MUST write a receipt under `target/ucan-canonical-file-tool-paths/`
- AND the receipt MUST hash source artifacts without embedding raw compact UCAN tokens, signing keys, prompts, provider payloads, file contents, or tool input bodies

#### Scenario: Closeout validation runs [r[ucan-basalt-daemon-auth.verification.canonical-file-path-closeout]]
- GIVEN implementation is complete
- WHEN the change is closed
- THEN focused Rust tests, checker receipt, Cairn validation/gates, sync/archive inspection, and diff checks MUST pass
