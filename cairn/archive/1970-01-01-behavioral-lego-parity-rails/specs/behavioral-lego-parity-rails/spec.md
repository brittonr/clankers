## ADDED Requirements

### Requirement: Lego rail inventory classifies checks [r[behavioral-lego-parity-rails.inventory]]

The SDK/Lego acceptance surface MUST inventory every architecture and parity rail with its owner, evidence class, requirement coverage, and replacement path for temporary string checks.

#### Scenario: Rail classification is explicit [r[behavioral-lego-parity-rails.inventory.classification]]
- GIVEN an acceptance script claims SDK/Lego boundary evidence
- WHEN the rail inventory is generated
- THEN the script MUST be classified as executable fixture, receipt verifier, AST/Cargo rail, or temporary string-presence check
- AND temporary string checks MUST name an owner, failure mode, and replacement path

### Requirement: Behavioral receipts name cases and outcomes [r[behavioral-lego-parity-rails.receipts]]

Executable lego rails MUST emit or verify deterministic receipts that identify what behavior was exercised.

#### Scenario: Receipt schema is complete [r[behavioral-lego-parity-rails.receipts.schema]]
- GIVEN a behavioral rail completes
- WHEN its receipt is inspected
- THEN the receipt MUST include case id, axis values, expected outcome, observed outcome, source artifacts, sanitized hashes, owner, and requirement ids
- AND volatile paths, credentials, provider payloads, raw hidden context, and environment secrets MUST NOT be required for comparison

### Requirement: Runtime and shell matrix rails execute fixtures [r[behavioral-lego-parity-rails.conversion]]

High-risk runtime extension service and shell adapter parity rails MUST execute or verify behavioral fixtures rather than only checking for symbol names.

#### Scenario: Runtime and shell matrices have behavior evidence [r[behavioral-lego-parity-rails.conversion.runtime-shell-matrices]]
- GIVEN runtime extension service or shell adapter parity acceptance runs
- WHEN the rail succeeds
- THEN it MUST have executed or verified fixture cases covering declared axes and outcomes
- AND removing the underlying behavioral assertion MUST fail the rail even if symbol names remain present

### Requirement: Negative fixtures are first-class [r[behavioral-lego-parity-rails.negative-fixtures]]

SDK/Lego rails MUST include fail-closed fixtures for high-risk absent, disabled, denied, and redacted paths.

#### Scenario: Fail-closed behavior is covered [r[behavioral-lego-parity-rails.negative-fixtures.fail-closed]]
- GIVEN provider/auth services are disabled, session stores are missing, capabilities deny tools, event metadata contains secret-like values, or forbidden transport leakage is introduced
- WHEN negative fixtures run
- THEN each case MUST fail closed with a typed safe outcome
- AND no live provider, router daemon, plugin subprocess, desktop dotdir, or transport fallback may start implicitly

### Requirement: Converted rails are wired into acceptance [r[behavioral-lego-parity-rails.acceptance]]

Converted behavioral rails MUST be part of the maintained embedded SDK acceptance surface.

#### Scenario: Receipt checks are wired [r[behavioral-lego-parity-rails.acceptance.wired-receipts]]
- GIVEN maintainers run the embedded SDK acceptance bundle or routine Nix/check surface
- WHEN converted rails are in scope
- THEN behavioral receipts MUST be generated or verified
- AND stale or missing receipts MUST fail acceptance with actionable owner diagnostics

### Requirement: Rail verification proves failure modes [r[behavioral-lego-parity-rails.verification]]

Rail implementation MUST include deterministic tests or fixtures showing that the rail fails when behavior or source artifacts drift.

#### Scenario: Rail failure fixtures catch drift [r[behavioral-lego-parity-rails.verification.rail-failure-fixtures]]
- GIVEN a fixture removes a required case, changes an expected outcome, omits an axis, or drops a source artifact hash
- WHEN the rail checks the fixture
- THEN it MUST fail with the missing case/axis/outcome/source artifact and owner requirement id

#### Scenario: Closeout validates converted rails [r[behavioral-lego-parity-rails.verification.closeout]]
- GIVEN implementation is complete
- WHEN focused validation runs
- THEN converted rail scripts, embedded SDK acceptance, Cairn validation/gates, and diff checks MUST pass
