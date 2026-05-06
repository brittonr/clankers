## ADDED Requirements

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
