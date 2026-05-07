## ADDED Requirements

### Requirement: Runtime extension service matrix coverage [r[tool-host-embedding.runtime-extension-service-matrix]]
The system MUST verify runtime extension services across an explicit matrix of absent, injected-success, injected-error, and denied states for auth, credential-pool, provider/router, plugin, and future extension placeholders where applicable.

#### Scenario: default-safe runtime fails closed independently [r[tool-host-embedding.runtime-extension-service-matrix.default-safe]]
- GIVEN a default-safe embedded runtime without injected extension services
- WHEN auth lookup, credential-pool selection, provider execution, plugin publication, or plugin execution is requested
- THEN each operation fails closed before hidden file reads, verifier writes, credential refresh persistence, daemon autostart, socket access, subprocess startup, or network provider execution

#### Scenario: mixed injected and absent services do not fall back ambiently [r[tool-host-embedding.runtime-extension-service-matrix.mixed-services]]
- GIVEN a matrix case injects only a subset of runtime extension services
- WHEN an operation for an injected service succeeds or fails
- THEN absent services are not discovered, started, or consulted implicitly
- THEN the result depends only on the explicitly injected service and request policy

#### Scenario: safe receipts are uniformly redacted [r[tool-host-embedding.runtime-extension-service-matrix.redaction]]
- GIVEN any runtime service matrix case returns a success, denial, or error receipt
- WHEN the receipt is serialized or logged for host inspection
- THEN it excludes raw prompts, provider request bodies, model output, credentials, refresh tokens, verifier contents, headers, environment values, raw auth files, raw plugin arguments, and raw plugin output
- THEN it includes only safe status, provider/account/tool identifiers, counts, and aggregate diagnostics

#### Scenario: side-effect sentinels prove negative claims [r[tool-host-embedding.runtime-extension-service-matrix.side-effect-sentinels]]
- GIVEN matrix tests install filesystem, socket, and fake-service counters before execution
- WHEN fail-closed or absent-service cases run
- THEN the test asserts that sentinels were not touched and fake services were not invoked outside the declared matrix state
