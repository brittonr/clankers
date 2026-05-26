# ADDED Requirements

### Requirement: dogfood rail [r[daemon-attach-reconnect-dogfood.local-reconnect]]

The dogfood rail MUST prove local daemon attach reconnect preserves or restores the intended session view.

#### Scenario: Local reconnect restores session view
- GIVEN a local daemon session exists with visible history
- WHEN the client detaches and reattaches
- THEN the attached UI shows the expected session/history without forking a new session

### Requirement: rail [r[daemon-attach-reconnect-dogfood.parity-reset]]

The rail MUST prove attach parity suppression state resets on reconnect.

#### Scenario: Parity tracker resets after reconnect
- GIVEN a slash/action acknowledgement was suppressed before detach
- WHEN the client reconnects
- THEN the first legitimate post-reconnect acknowledgement is not hidden by stale suppression budget

### Requirement: rail [r[daemon-attach-reconnect-dogfood.deterministic-provider]]

The rail MUST avoid live model dependency.

#### Scenario: Provider behavior is deterministic
- GIVEN the reconnect dogfood runs in CI-like local conditions
- WHEN the provider seam is exercised
- THEN the proof uses a stub or recorded deterministic provider flow rather than live credentials

### Requirement: rail [r[daemon-attach-reconnect-dogfood.cleanup-receipt]]

The rail MUST emit a cleanup-aware receipt.

#### Scenario: Daemon dogfood cleanup is verified
- GIVEN the dogfood rail completes
- WHEN the receipt is written
- THEN it records daemon/session identifiers, assertions, and cleanup status under target
