# Steel Self-Mutation Policy Delta

## Requirement: Host functions preserve Rust authority [r[steel-self-mutation-policy.host-functions]]
Steel scripts MAY request mutation only through typed Clankers host functions. Those functions MUST call Rust enforcement code and MUST NOT expose raw filesystem, process, network, git, credential, provider, daemon, TUI, or native-tool authority to Steel.

### Scenario: host functions apply only through Rust [r[steel-self-mutation-policy.host-functions.apply-through-rust]]
- GIVEN Steel emits a typed live-mutation host-function request
- WHEN the request is evaluated
- THEN Clankers MUST route the request through Rust-owned validation, staging, gate execution, receipt writing, promotion, and rollback code
- AND Steel MUST receive only typed proposal/receipt data rather than direct handles to files, shell commands, git, providers, credentials, daemon control, or network sockets

### Scenario: raw host writes are denied [r[steel-self-mutation-policy.host-functions.raw-write-denied]]
- GIVEN a Steel script attempts to write a file, spawn a process, mutate git state, access credentials, call a provider, or open network access outside an approved typed host function
- WHEN the host bridge handles the request
- THEN Clankers MUST deny it before the side effect
- AND the denial receipt MUST name the forbidden authority class without echoing secret data or raw request bodies

## Requirement: Mutation preflight and receipts are deterministic [r[steel-self-mutation-policy.receipts-and-preflight]]
Before live writes, Clankers MUST record deterministic preflight evidence and after any attempted write it MUST emit a receipt that supports audit, verification, and rollback.

### Scenario: preflight records mutation decision inputs [r[steel-self-mutation-policy.receipts-and-preflight.preflight]]
- GIVEN Rust receives a Steel live-mutation proposal
- WHEN preflight runs
- THEN Clankers MUST record the normalized target paths, expected before hash, patch hash, selected gates, activation policy, authority-change summary, policy identity, and script identity before any live write
- AND preflight MUST reject malformed schemas, stale before hashes, path escapes, missing required gates, and authority-kernel widening before staging or promotion

### Scenario: receipts are safe to disclose [r[steel-self-mutation-policy.receipts-and-preflight.safe-receipt]]
- GIVEN a live-mutation request contains prompts, credentials, UCAN proofs, provider payloads, secret paths, raw patches, or transcript material
- WHEN Clankers emits a mutation receipt
- THEN the receipt MUST contain bounded hashes and safe metadata only
- AND raw sensitive material MUST NOT be written to session-visible receipts, docs evidence, or review artifacts

## Requirement: Verification and rollback gate success [r[steel-self-mutation-policy.verification-and-rollback]]
A live mutation MUST NOT be reported as successful or promoted to commit/application success unless required verification passes, and rollback MUST guard against clobbering operator edits.

### Scenario: failed verification blocks promotion [r[steel-self-mutation-policy.verification-and-rollback.failed-verification]]
- GIVEN an orchestration-pack mutation stages successfully but at least one required verification gate fails or is missing
- WHEN Clankers evaluates promotion
- THEN it MUST leave the live pack unchanged, mark the mutation failed validation, and record the failed gate receipt hashes
- AND it MUST NOT report the mutation as successful, activated, or committed

### Scenario: guarded rollback restores only matching backups [r[steel-self-mutation-policy.verification-and-rollback.guarded-rollback]]
- GIVEN rollback is requested for a promoted orchestration-pack mutation
- WHEN the current hash or backup hash differs from the receipt
- THEN Clankers MUST deny rollback before writes
- AND only a current hash matching the recorded post-apply hash plus a backup hash matching the recorded pre-apply hash MAY restore files

## Requirement: Fixtures prove allowed and denied behavior [r[steel-self-mutation-policy.verification-fixtures]]
Implementation MUST include deterministic positive and negative fixtures for Nickel policy validation, UCAN authority checks, host-function enforcement, receipt redaction, verification gating, and rollback guards.

### Scenario: positive fixtures prove safe mutation path [r[steel-self-mutation-policy.verification-fixtures.positive]]
- GIVEN fixtures for a valid script/gate update with matching before hash, required gates, no authority widening, and passing gate receipts
- WHEN focused verification runs
- THEN the fixture MUST stage in isolation, promote only after gates pass, record a rollback reference, and activate only on explicit reload or a later turn

### Scenario: negative fixtures prove denied mutation paths [r[steel-self-mutation-policy.verification-fixtures.negative]]
- GIVEN fixtures for path escape, stale before hash, authority widening, required gate removal, failed validation, malformed patch schema, raw write attempt, unsafe receipt content, and stale rollback target
- WHEN focused verification runs
- THEN every fixture MUST fail before forbidden writes, authority widening, unsafe receipt emission, promotion, or unsafe rollback occurs
