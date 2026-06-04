## Purpose

Context references expand documented `@` prompt references into bounded local, git, image, and policy-enabled remote context while recording safe provenance metadata for session replay and debugging.
## Requirements
### Requirement: Context References Capability [r[context-references.capability]]
The system MUST expand documented local `@` references in prompts into files, directories, and image attachments, and MUST return explicit unsupported-reference output for URL, git-diff, remote, or session-artifact references until those reference kinds are implemented.

#### Scenario: Primary path succeeds [r[context-references.scenario.primary-path]]
- GIVEN clankers is configured for the capability
- WHEN the user or agent invokes the documented primary path
- THEN clankers performs the operation and returns a structured, user-visible result

#### Scenario: Unsupported configuration is explicit [r[context-references.scenario.unsupported-config]]
- GIVEN the user invokes the capability without required configuration or platform support
- WHEN clankers cannot safely proceed
- THEN clankers MUST return an actionable error instead of silently falling back or dropping work

### Requirement: Context References Session Observability [r[context-references.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[context-references.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes status, timing or backend identity when useful, and redacted error details when applicable

### Requirement: Context References Verification [r[context-references.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[context-references.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure

### Requirement: Expanded Context Reference Kinds [r[context-references.expanded-kinds]]
The system MUST support additional reference kinds for git diffs, URLs, session history, and session artifacts while preserving explicit unsupported outcomes for unavailable kinds.

#### Scenario: Git diff reference [r[context-references.expanded-kinds.scenario.git-diff-reference]]
- GIVEN the prompt contains @diff or a scoped diff reference in a git checkout
- WHEN the prompt is dispatched
- THEN clankers injects a bounded textual diff summary/content and records diff metadata

#### Scenario: Url reference [r[context-references.expanded-kinds.scenario.url-reference]]
- GIVEN the prompt contains an HTTP or HTTPS reference and network access is permitted by policy
- WHEN the prompt is dispatched
- THEN clankers fetches bounded readable content or records an actionable fetch error

### Requirement: Context Reference Provenance and Bounds [r[context-references.provenance]]
The system MUST record provenance, size bounds, and expansion status for each reference without storing raw secrets in metadata.

#### Scenario: Oversized reference [r[context-references.provenance.scenario.oversized-reference]]
- GIVEN a reference expands beyond configured limits
- WHEN expansion runs
- THEN clankers truncates or rejects according to policy and records the limit decision

#### Scenario: Metadata safe [r[context-references.provenance.scenario.metadata-safe]]
- GIVEN a reference resolves to content that may contain secrets
- WHEN session metadata is persisted
- THEN metadata includes kind, source, status, size, hashes or paths when safe, and error class without raw content or credentials
