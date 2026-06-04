Artifact-Type: verification-log
Evidence-ID: engine-retry-stop-policy.reducer-tests-red
Task-ID: 1.10
Covers: embeddable-agent-engine.reducer-retry-tests, embeddable-agent-engine.reducer-budget-token-tests
Creator: pi
Created: 2026-04-25T01:23:10Z
Status: EXPECTED-FAIL
Command: RUSTC_WRAPPER= cargo test -p clankers-engine

Output:

```text
   Compiling proc-macro2 v1.0.106
   Compiling quote v1.0.45
   Compiling unicode-ident v1.0.24
   Compiling thiserror v2.0.18
   Compiling heck v0.5.0
   Compiling icu_normalizer_data v2.2.0
   Compiling icu_properties_data v2.2.0
   Compiling syn v2.0.117
   Compiling synstructure v0.13.2
   Compiling darling_core v0.20.11
   Compiling darling_core v0.23.0
   Compiling serde_derive v1.0.228
   Compiling tokio-macros v2.7.0
   Compiling futures-macro v0.3.32
   Compiling zeroize_derive v1.4.3
   Compiling zerofrom-derive v0.1.7
   Compiling yoke-derive v0.8.2
   Compiling thiserror-impl v2.0.18
   Compiling zerovec-derive v0.11.3
   Compiling displaydoc v0.2.5
   Compiling tracing-attributes v0.1.31
   Compiling derive_more-impl v2.1.1
   Compiling strum_macros v0.27.2
   Compiling n0-error-macros v0.1.3
   Compiling spez v0.1.2
   Compiling openssl-macros v0.1.1
   Compiling pin-project-internal v1.1.11
   Compiling async-trait v0.1.89
   Compiling curve25519-dalek-derive v0.1.1
   Compiling enum-as-inner v0.6.1
   Compiling postcard-derive v0.2.2
   Compiling enum-assoc v1.3.0
   Compiling num_enum_derive v0.7.6
   Compiling iroh-metrics-derive v0.4.1
   Compiling thiserror-impl v1.0.69
   Compiling zeroize v1.8.2
   Compiling tokio v1.52.1
   Compiling rustls-pki-types v1.14.0
   Compiling aws-lc-rs v1.16.3
   Compiling der v0.8.0
   Compiling futures-util v0.3.32
   Compiling num_enum v0.7.6
   Compiling zerofrom v0.1.7
   Compiling n0-error v0.1.3
   Compiling yoke v0.8.2
   Compiling webpki-roots v1.0.7
   Compiling rustls-native-certs v0.8.3
   Compiling pin-project v1.1.11
   Compiling thiserror v1.0.69
   Compiling zerovec v0.11.6
   Compiling zerotrie v0.2.4
   Compiling darling_macro v0.20.11
   Compiling kasuari v0.4.12
   Compiling cobs v0.3.0
   Compiling darling_macro v0.23.0
   Compiling darling v0.20.11
   Compiling derive_builder_core v0.20.2
   Compiling darling v0.23.0
   Compiling tinystr v0.8.3
   Compiling potential_utf v0.1.5
   Compiling instability v0.3.12
   Compiling icu_collections v2.2.0
   Compiling icu_locale_core v2.2.0
   Compiling spki v0.8.0
   Compiling strum v0.27.2
   Compiling rustls-webpki v0.103.13
   Compiling ratatui-core v0.1.0
   Compiling pkcs8 v0.11.0-rc.11
   Compiling derive_builder_macro v0.20.2
   Compiling derive_more v2.1.1
   Compiling icu_provider v2.2.0
   Compiling crossterm v0.29.0
   Compiling derive_builder v0.20.2
   Compiling vergen-lib v9.1.0
   Compiling vergen-lib v0.1.6
   Compiling serde v1.0.228
   Compiling icu_normalizer v2.2.0
   Compiling icu_properties v2.2.0
   Compiling vergen v9.1.0
   Compiling vergen-gitcl v1.0.8
   Compiling ratatui-widgets v0.3.0
   Compiling iroh-relay v0.96.1
   Compiling portable-atomic v1.13.1
   Compiling curve25519-dalek v5.0.0-pre.1
   Compiling heapless v0.7.17
   Compiling ed25519 v3.0.0-rc.4
   Compiling serde_urlencoded v0.7.1
   Compiling chrono v0.4.44
   Compiling ratatui-crossterm v0.1.0
   Compiling postcard v1.1.3
   Compiling once_cell v1.21.4
   Compiling moka v0.12.15
   Compiling futures-executor v0.3.32
   Compiling tracing-core v0.1.36
   Compiling rustls v0.23.39
   Compiling openssl v0.10.78
   Compiling ntimestamp v1.0.0
   Compiling idna_adapter v1.2.1
   Compiling futures v0.3.32
   Compiling idna v1.1.0
   Compiling ratatui-macros v0.7.0
   Compiling tracing v0.1.44
   Compiling url v2.5.8
   Compiling ed25519-dalek v3.0.0-pre.1
   Compiling ratatui v0.30.0
   Compiling rat-leaderkey v0.1.0 (ssh://git@github.com/brittonr/subwayrat.git?rev=d930e8ce693bc3761eff5a4ed4a8bd109b2cd7fc#d930e8ce)
   Compiling rat-markdown v0.1.0 (ssh://git@github.com/brittonr/subwayrat.git?rev=d930e8ce693bc3761eff5a4ed4a8bd109b2cd7fc#d930e8ce)
   Compiling rat-branches v0.1.0 (ssh://git@github.com/brittonr/subwayrat.git?rev=d930e8ce693bc3761eff5a4ed4a8bd109b2cd7fc#d930e8ce)
   Compiling iroh-quinn-udp v0.8.0
   Compiling iroh-metrics v0.38.3
   Compiling clanker-tui-types v0.1.0 (https://github.com/brittonr/clanker-tui-types#0c72646a)
   Compiling attohttpc v0.30.1
   Compiling iroh-base v0.96.1
   Compiling tokio-util v0.7.18
   Compiling tower v0.5.3
   Compiling netlink-sys v0.8.8
   Compiling async-compat v0.2.5
   Compiling acto v0.8.0
   Compiling backon v1.6.0
   Compiling fs4 v0.13.1
   Compiling netlink-proto v0.12.0
   Compiling netdev v0.40.1
   Compiling tower-http v0.6.8
   Compiling h2 v0.4.13
   Compiling n0-future v0.3.2
   Compiling tokio-stream v0.1.18
   Compiling n0-watcher v0.6.1
   Compiling netwatch v0.14.0
   Compiling native-tls v0.2.18
   Compiling tokio-native-tls v0.3.1
   Compiling tokio-rustls v0.26.4
   Compiling rustls-platform-verifier v0.6.2
   Compiling iroh-quinn-proto v0.15.1
   Compiling tokio-websockets v0.12.3
   Compiling hyper v1.9.0
   Compiling hickory-proto v0.25.2
   Compiling hyper-util v0.1.20
   Compiling iroh-quinn v0.16.1
   Compiling hyper-rustls v0.27.9
   Compiling hyper-tls v0.6.0
   Compiling igd-next v0.16.2
   Compiling reqwest v0.12.28
   Compiling reqwest v0.13.2
   Compiling hickory-resolver v0.25.2
   Compiling swarm-discovery v0.5.0
   Compiling portmapper v0.14.0
   Compiling pkarr v5.0.4
   Compiling reqwest-eventsource v0.6.0
   Compiling iroh v0.96.1
   Compiling clanker-router v0.1.0 (/home/brittonr/git/clankers/vendor/clanker-router)
   Compiling clanker-message v0.1.0 (https://github.com/brittonr/clanker-message#58bfda34)
   Compiling clankers-provider v0.1.0 (/home/brittonr/git/clankers/crates/clankers-provider)
   Compiling clankers-engine v0.1.0 (/home/brittonr/git/clankers/crates/clankers-engine)
error[E0425]: cannot find type `EngineTerminalFailure` in this scope
   --> crates/clankers-engine/src/lib.rs:639:79
    |
639 |     fn engine_failure(message: &str, status: Option<u16>, retryable: bool) -> EngineTerminalFailure {
    |                                                                               ^^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0422]: cannot find struct, variant or union type `EngineTerminalFailure` in this scope
   --> crates/clankers-engine/src/lib.rs:640:9
    |
640 |         EngineTerminalFailure {
    |         ^^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find type `EngineTerminalFailure` in this scope
   --> crates/clankers-engine/src/lib.rs:647:44
    |
647 |     fn retryable_failure(message: &str) -> EngineTerminalFailure {
    |                                            ^^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find type `EngineTerminalFailure` in this scope
   --> crates/clankers-engine/src/lib.rs:651:48
    |
651 |     fn non_retryable_failure(message: &str) -> EngineTerminalFailure {
    |                                                ^^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0425]: cannot find type `EngineTerminalFailure` in this scope
   --> crates/clankers-engine/src/lib.rs:661:70
    |
661 |     fn model_failed_input(request_id: &EngineCorrelationId, failure: EngineTerminalFailure) -> EngineInput {
    |                                                                      ^^^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0560]: struct `EnginePromptSubmission` has no field named `model_request_slot_budget`
   --> crates/clankers-engine/src/lib.rs:620:13
    |
620 |             model_request_slot_budget: DEFAULT_TEST_MODEL_REQUEST_SLOT_BUDGET,
    |             ^^^^^^^^^^^^^^^^^^^^^^^^^ `EnginePromptSubmission` does not have this field
    |
    = note: all struct fields are already assigned

error[E0599]: no variant named `RetryReady` found for enum `EngineInput`
   --> crates/clankers-engine/src/lib.rs:656:22
    |
105 | pub enum EngineInput {
    | -------------------- variant `RetryReady` not found here
...
656 |         EngineInput::RetryReady {
    |                      ^^^^^^^^^^ variant not found in `EngineInput`

error[E0559]: variant `EngineInput::ModelFailed` has no field named `failure`
   --> crates/clankers-engine/src/lib.rs:664:13
    |
664 |             failure,
    |             ^^^^^^^ `EngineInput::ModelFailed` does not have this field
    |
    = note: available fields are: `error`

error[E0599]: no variant named `ScheduleRetry` found for enum `EngineEffect`
   --> crates/clankers-engine/src/lib.rs:693:31
    |
 98 | pub enum EngineEffect {
    | --------------------- variant `ScheduleRetry` not found here
...
693 |                 EngineEffect::ScheduleRetry { request_id, delay } => Some((request_id, *delay)),
    |                               ^^^^^^^^^^^^^ variant not found in `EngineEffect`

error[E0599]: no variant named `ScheduleRetry` found for enum `EngineEffect`
   --> crates/clankers-engine/src/lib.rs:710:59
    |
 98 | pub enum EngineEffect {
    | --------------------- variant `ScheduleRetry` not found here
...
710 |             .all(|effect| !matches!(effect, EngineEffect::ScheduleRetry { .. }))
    |                                                           ^^^^^^^^^^^^^ variant not found in `EngineEffect`

error[E0609]: no field `terminal_failure` on type `&EngineOutcome`
   --> crates/clankers-engine/src/lib.rs:742:25
    |
742 |         assert!(outcome.terminal_failure.is_none());
    |                         ^^^^^^^^^^^^^^^^ unknown field
    |
    = note: available fields are: `next_state`, `effects`, `rejection`

error[E0599]: no variant, associated function, or constant named `WaitingForRetry` found for enum `EngineTurnPhase` in the current scope
   --> crates/clankers-engine/src/lib.rs:784:67
    |
 22 | pub enum EngineTurnPhase {
    | ------------------------ variant, associated function, or constant `WaitingForRetry` not found for this enum
...
784 |         assert_eq!(first_retry.next_state.phase, EngineTurnPhase::WaitingForRetry);
    |                                                                   ^^^^^^^^^^^^^^^ variant, associated function, or constant not found in `EngineTurnPhase`
    |
help: there is a variant with a similar name
    |
784 -         assert_eq!(first_retry.next_state.phase, EngineTurnPhase::WaitingForRetry);
784 +         assert_eq!(first_retry.next_state.phase, EngineTurnPhase::WaitingForModel);
    |

error[E0369]: binary operation `==` cannot be applied to type `Vec<EngineMessage>`
   --> crates/clankers-engine/src/lib.rs:786:9
    |
786 |         assert_eq!(first_retry.next_state.messages, original_messages);
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |         |
    |         Vec<EngineMessage>
    |         Vec<EngineMessage>
    |
note: an implementation of `PartialEq` might be missing for `EngineMessage`
   --> crates/clankers-engine/src/lib.rs:30:1
    |
 30 | pub struct EngineMessage {
    | ^^^^^^^^^^^^^^^^^^^^^^^^ must implement `PartialEq`
help: consider annotating `EngineMessage` with `#[derive(PartialEq)]`
    |
 30 + #[derive(PartialEq)]
 31 | pub struct EngineMessage {
    |

error[E0609]: no field `terminal_failure` on type `EngineOutcome`
   --> crates/clankers-engine/src/lib.rs:787:29
    |
787 |         assert!(first_retry.terminal_failure.is_none());
    |                             ^^^^^^^^^^^^^^^^ unknown field
    |
    = note: available fields are: `next_state`, `effects`, `rejection`

error[E0369]: binary operation `==` cannot be applied to type `Vec<EngineMessage>`
   --> crates/clankers-engine/src/lib.rs:798:9
    |
798 |         assert_eq!(retry_ready.next_state.messages, original_messages);
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |         |
    |         Vec<EngineMessage>
    |         Vec<EngineMessage>
    |
note: an implementation of `PartialEq` might be missing for `EngineMessage`
   --> crates/clankers-engine/src/lib.rs:30:1
    |
 30 | pub struct EngineMessage {
    | ^^^^^^^^^^^^^^^^^^^^^^^^ must implement `PartialEq`
help: consider annotating `EngineMessage` with `#[derive(PartialEq)]`
    |
 30 + #[derive(PartialEq)]
 31 | pub struct EngineMessage {
    |

error[E0369]: binary operation `==` cannot be applied to type `Vec<EngineMessage>`
   --> crates/clankers-engine/src/lib.rs:866:9
    |
866 |         assert_eq!(terminal.next_state.messages, original_messages);
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |         |
    |         Vec<EngineMessage>
    |         Vec<EngineMessage>
    |
note: an implementation of `PartialEq` might be missing for `EngineMessage`
   --> crates/clankers-engine/src/lib.rs:30:1
    |
 30 | pub struct EngineMessage {
    | ^^^^^^^^^^^^^^^^^^^^^^^^ must implement `PartialEq`
help: consider annotating `EngineMessage` with `#[derive(PartialEq)]`
    |
 30 + #[derive(PartialEq)]
 31 | pub struct EngineMessage {
    |

error[E0609]: no field `terminal_failure` on type `EngineOutcome`
   --> crates/clankers-engine/src/lib.rs:867:29
    |
867 |         assert_eq!(terminal.terminal_failure, Some(retryable_failure("third failure")));
    |                             ^^^^^^^^^^^^^^^^ unknown field
    |
    = note: available fields are: `next_state`, `effects`, `rejection`

error[E0369]: binary operation `==` cannot be applied to type `Vec<EngineMessage>`
   --> crates/clankers-engine/src/lib.rs:886:9
    |
886 |         assert_eq!(terminal.next_state.messages, original_messages);
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |         |
    |         Vec<EngineMessage>
    |         Vec<EngineMessage>
    |
note: an implementation of `PartialEq` might be missing for `EngineMessage`
   --> crates/clankers-engine/src/lib.rs:30:1
    |
 30 | pub struct EngineMessage {
    | ^^^^^^^^^^^^^^^^^^^^^^^^ must implement `PartialEq`
help: consider annotating `EngineMessage` with `#[derive(PartialEq)]`
    |
 30 + #[derive(PartialEq)]
 31 | pub struct EngineMessage {
    |

error[E0609]: no field `terminal_failure` on type `EngineOutcome`
   --> crates/clankers-engine/src/lib.rs:887:29
    |
887 |         assert_eq!(terminal.terminal_failure, Some(non_retryable_failure("bad request")));
    |                             ^^^^^^^^^^^^^^^^ unknown field
    |
    = note: available fields are: `next_state`, `effects`, `rejection`

error[E0369]: binary operation `==` cannot be applied to type `Vec<EngineMessage>`
   --> crates/clankers-engine/src/lib.rs:953:9
    |
953 |         assert_eq!(first_retry.next_state.messages, original_messages);
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |         |
    |         Vec<EngineMessage>
    |         Vec<EngineMessage>
    |
note: an implementation of `PartialEq` might be missing for `EngineMessage`
   --> crates/clankers-engine/src/lib.rs:30:1
    |
 30 | pub struct EngineMessage {
    | ^^^^^^^^^^^^^^^^^^^^^^^^ must implement `PartialEq`
help: consider annotating `EngineMessage` with `#[derive(PartialEq)]`
    |
 30 + #[derive(PartialEq)]
 31 | pub struct EngineMessage {
    |

error[E0369]: binary operation `==` cannot be applied to type `Vec<EngineMessage>`
   --> crates/clankers-engine/src/lib.rs:954:9
    |
954 |         assert_eq!(second_retry.next_state.messages, original_messages);
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |         |
    |         Vec<EngineMessage>
    |         Vec<EngineMessage>
    |
note: an implementation of `PartialEq` might be missing for `EngineMessage`
   --> crates/clankers-engine/src/lib.rs:30:1
    |
 30 | pub struct EngineMessage {
    | ^^^^^^^^^^^^^^^^^^^^^^^^ must implement `PartialEq`
help: consider annotating `EngineMessage` with `#[derive(PartialEq)]`
    |
 30 + #[derive(PartialEq)]
 31 | pub struct EngineMessage {
    |

error[E0369]: binary operation `==` cannot be applied to type `Vec<EngineMessage>`
   --> crates/clankers-engine/src/lib.rs:955:9
    |
955 |         assert_eq!(terminal.next_state.messages, original_messages);
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |         |
    |         Vec<EngineMessage>
    |         Vec<EngineMessage>
    |
note: an implementation of `PartialEq` might be missing for `EngineMessage`
   --> crates/clankers-engine/src/lib.rs:30:1
    |
 30 | pub struct EngineMessage {
    | ^^^^^^^^^^^^^^^^^^^^^^^^ must implement `PartialEq`
help: consider annotating `EngineMessage` with `#[derive(PartialEq)]`
    |
 30 + #[derive(PartialEq)]
 31 | pub struct EngineMessage {
    |

error[E0609]: no field `model_request_slot_budget` on type `EnginePromptSubmission`
   --> crates/clankers-engine/src/lib.rs:961:20
    |
961 |         submission.model_request_slot_budget = TWO_MODEL_REQUEST_SLOT_BUDGET;
    |                    ^^^^^^^^^^^^^^^^^^^^^^^^^ unknown field
    |
    = note: available fields are: `messages`, `model`, `system_prompt`, `max_tokens`, `temperature` ... and 5 others

error[E0609]: no field `terminal_failure` on type `EngineOutcome`
    --> crates/clankers-engine/src/lib.rs:1008:27
     |
1008 |         assert!(exhausted.terminal_failure.is_none());
     |                           ^^^^^^^^^^^^^^^^ unknown field
     |
     = note: available fields are: `next_state`, `effects`, `rejection`

error[E0609]: no field `model_request_slot_budget` on type `EnginePromptSubmission`
    --> crates/clankers-engine/src/lib.rs:1030:20
     |
1030 |         submission.model_request_slot_budget = ZERO_MODEL_REQUEST_SLOT_BUDGET;
     |                    ^^^^^^^^^^^^^^^^^^^^^^^^^ unknown field
     |
     = note: available fields are: `messages`, `model`, `system_prompt`, `max_tokens`, `temperature` ... and 5 others

error[E0599]: no variant, associated function, or constant named `InvalidBudget` found for enum `EngineRejection` in the current scope
    --> crates/clankers-engine/src/lib.rs:1034:81
     |
 132 | pub enum EngineRejection {
     | ------------------------ variant, associated function, or constant `InvalidBudget` not found for this enum
...
1034 |         assert_rejected_without_state_change(&state, &outcome, EngineRejection::InvalidBudget);
     |                                                                                 ^^^^^^^^^^^^^ variant, associated function, or constant not found in `EngineRejection`

error[E0609]: no field `terminal_failure` on type `EngineOutcome`
    --> crates/clankers-engine/src/lib.rs:1054:25
     |
1054 |         assert!(outcome.terminal_failure.is_none());
     |                         ^^^^^^^^^^^^^^^^ unknown field
     |
     = note: available fields are: `next_state`, `effects`, `rejection`

Some errors have detailed explanations: E0369, E0422, E0425, E0559, E0560, E0599, E0609.
For more information about an error, try `rustc --explain E0369`.
error: could not compile `clankers-engine` (lib test) due to 27 previous errors

```
