## ADDED Requirements

### Requirement: Pi-observable dogfood surface [r[clankers-observable-soak-rails.pi-observable-surface]]

Clankers MUST expose maintained local dogfood rails that let pi or a human operator observe the critical interactive surfaces without live model credentials: standalone token/thinking streaming, daemon/attach streaming abort-and-follow-up, daemon attach reconnect, and background-process TUI visibility.

#### Scenario: Critical surfaces have named receipts [r[clankers-observable-soak-rails.pi-observable-surface.named-receipts]]
- GIVEN an operator needs to inspect Clankers interactive readiness
- WHEN they run the documented dogfood selectors
- THEN each selector writes a deterministic local receipt under `target/dogfood/`
- AND the receipt records the selector-specific pass fields and artifacts needed to audit the claim

#### Scenario: Live credentials are not required [r[clankers-observable-soak-rails.pi-observable-surface.local-stubs]]
- GIVEN the dogfood rails exercise model streaming or assistant output
- WHEN the rails run in a local developer environment
- THEN they use deterministic local provider stubs rather than OpenAI/OAuth or other live credentials
- AND the receipts record deterministic-provider facts

### Requirement: Daemon attach streaming abort proof [r[clankers-observable-soak-rails.daemon-attach-abort]]

The daemon/attach streaming rail MUST prove that follow-up input submitted while an attached daemon session is streaming aborts the running turn and reaches the provider before the interrupted stream completes.

#### Scenario: Follow-up is accepted before provider completion [r[clankers-observable-soak-rails.daemon-attach-abort.followup-before-completion]]
- GIVEN a real attached TUI is connected to an isolated local daemon session
- AND the first provider request is a long synthetic stream
- WHEN a follow-up prompt is submitted while the TUI shows active streaming
- THEN the receipt records `mid_stream_abort_processed_before_provider_returned: true`
- AND it records `followup_request_started_before_stream_completed: true`
- AND it records `busy_rejection_visible: false`

#### Scenario: Attached rail leaves cleanup evidence [r[clankers-observable-soak-rails.daemon-attach-abort.cleanup]]
- GIVEN the daemon/attach streaming rail finishes
- WHEN it writes its receipt
- THEN it uses schema `clankers.daemon_attach_streaming_abort_dogfood.receipt.v1`
- AND it records screen artifacts, timing fields, provider request count, and `daemon_cleaned_up: true`

### Requirement: Soak harness repeats observable rails [r[clankers-observable-soak-rails.soak-harness]]

The test harness MUST expose a `soak` mode that repeats selected dogfood rails for flake hunting while preserving per-step logs and the aggregate harness receipt.

#### Scenario: Streaming soak expands deterministically [r[clankers-observable-soak-rails.soak-harness.streaming-expansion]]
- GIVEN an operator runs `./scripts/test-harness.sh soak streaming 2`
- WHEN the harness runs in dry-run or real mode
- THEN it schedules exactly two iterations of `streaming-tokens`
- AND exactly two iterations of `daemon-attach-streaming-abort`
- AND each step is visible in the harness receipt with a unique iteration label

#### Scenario: Soak iteration bounds fail closed [r[clankers-observable-soak-rails.soak-harness.iteration-bounds]]
- GIVEN an operator supplies a soak iteration count
- WHEN the count is not a positive integer or is greater than 50
- THEN the harness exits nonzero before running dogfood rails
- AND the error names the iteration-count constraint

### Requirement: Release docs name soak and per-rail criteria [r[clankers-observable-soak-rails.release-docs]]

Release-readiness documentation and README snippets MUST name the focused dogfood rails, the soak mode, and the receipt fields required before claiming stability for streaming or daemon/attach input.

#### Scenario: Docs prevent overclaiming [r[clankers-observable-soak-rails.release-docs.no-overclaim]]
- GIVEN a developer reads the release-readiness checklist
- WHEN it describes soak evidence
- THEN it says soak is flake-hunting evidence, not a replacement for inspecting per-rail receipt fields
- AND it keeps the broader public-production non-claim language intact
