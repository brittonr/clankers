## MODIFIED Requirements

### Requirement: Receipts expose artifact provenance safely

The system MUST include artifact hashes in model requests, tool calls, session blocks, replay records, authorization decisions, and review receipts where those artifacts influenced execution. UCAN proof-chain, grant, caveat-policy, replay-admission, and revocation metadata MUST be recorded only as safe identifiers, hashes, statuses, and redacted summaries.

#### Scenario: replay resolves original artifacts

- GIVEN a persisted session contains model, tool, authorization, and replay receipts with artifact hashes
- WHEN replay or review inspection loads the session
- THEN it can resolve the exact prompt, tool descriptor, request envelope, session block, redacted authorization metadata, and policy artifacts that were used
- THEN missing artifacts are reported as missing provenance rather than silently ignored

#### Scenario: inspect output stays redacted

- GIVEN an artifact includes fields classified as secret, raw compact UCAN token, raw provider payload, credential, header, or environment value
- WHEN a user inspects the artifact hash through CLI/TUI/review output
- THEN Clankers returns safe metadata and redacted payload fields only
- THEN raw credentials, compact tokens, headers, environment values, and unredacted provider bodies are not printed
