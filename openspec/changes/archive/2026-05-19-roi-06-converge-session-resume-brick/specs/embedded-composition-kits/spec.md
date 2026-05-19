## ADDED Requirements

### Requirement: Session/resume brick convergence [r[embedded-composition-kits.session-resume-brick]]

The system MUST gather comparable host-owned session/resume evidence before promoting a reusable public session API.

#### Scenario: Multiple products preserve restored context [r[embedded-composition-kits.session-resume-brick.multi-product]]

- GIVEN two or more product-style embeddings persist and resume embedded sessions
- WHEN their follow-up turns run
- THEN each MUST prove restored user/tool/assistant context reaches the next `EngineModelRequest` in deterministic order
- THEN each product MUST own its storage DTOs and persistence I/O unless a later OpenSpec promotes a reusable session trait

#### Scenario: Missing-session and fork prevention are explicit [r[embedded-composition-kits.session-resume-brick.fail-closed]]

- GIVEN a product requests a missing or stale session id
- WHEN restore is attempted
- THEN the embedding MUST fail closed before model/tool execution or explicitly create a new session through a separate product-owned path
- THEN it MUST NOT silently fork a replacement session or read Clankers JSONL/DB/session shell state

#### Scenario: Resume evidence is content addressed [r[embedded-composition-kits.session-resume-brick.blake3-evidence]]

- GIVEN a product emits session/resume evidence
- WHEN the dogfood rail completes
- THEN it SHOULD include BLAKE3 hashes for sanitized transcripts, restored-context fixtures, session DTO schema examples, and turn receipts
- THEN privacy-sensitive data MUST be redacted before hashing when evidence is committed

#### Scenario: Schema contracts are optional authoring aids [r[embedded-composition-kits.session-resume-brick.nickel-schema]]

- GIVEN a product wants a checked session DTO schema or migration policy
- WHEN it authors one in Nickel
- THEN Nickel contracts MAY validate product-owned schema examples and migration fields
- THEN Clankers generic SDK crates MUST NOT take ownership of product persistence, migrations, or database access
