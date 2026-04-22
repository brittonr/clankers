## ADDED Requirements

### Requirement: Embedded runtime feedback adapters MUST stay shell-native
ID: no.std.functional.core.embedded.feedback.shell.native.adapters
The system MUST expose controller-owned shell-native adapters for embedded prompt completion and follow-up dispatch/completion feedback so root runtime files do not construct `clankers_core` feedback types directly for the migrated slice.

#### Scenario: controller-owned pending-work identities cross the shell boundary
ID: no.std.functional.core.embedded.feedback.shell.native.adapters.controller-owned-pending-work-identities-cross-the-shell-boundary
- **WHEN** embedded runtime code receives `PostPromptAction::ContinueLoop` or `PostPromptAction::RunAutoTest`
- **THEN** the correlation token returned to runtime code is a controller-owned pending-work identity rather than a raw `clankers_core::CoreEffectId`
- **THEN** conversion back to core-owned correlation data happens inside `crates/clankers-controller`

#### Scenario: embedded runtime avoids direct core feedback construction
ID: no.std.functional.core.embedded.feedback.shell.native.adapters.embedded-runtime-avoids-direct-core-feedback-construction
- **WHEN** `src/modes/event_loop_runner/mod.rs` reports accepted or rejected follow-up dispatch, successful or failed embedded prompt completion, or successful or failed follow-up completion
- **THEN** it calls controller-owned shell-native APIs
- **THEN** it does not construct `clankers_core::CompletionStatus`, `clankers_core::FollowUpDispatchStatus`, `clankers_core::CoreFailure`, or raw core correlation IDs directly in non-test code

#### Scenario: embedded feedback behavior remains aligned
ID: no.std.functional.core.embedded.feedback.shell.native.adapters.embedded-feedback-behavior-remains-aligned
- **WHEN** embedded shells report accepted or rejected follow-up dispatch or successful or failed prompt/follow-up completion through the shell-native controller adapters for either `PostPromptAction::ContinueLoop` or `PostPromptAction::RunAutoTest`
- **THEN** accepted dispatch plus successful follow-up completion still advances loop state only after the follow-up prompt actually finishes
- **THEN** rejected dispatch, failed follow-up completion, and failed prompt completion still preserve queued-prompt replay rules plus loop-failure and error surfacing behavior on both follow-up branches
- **THEN** rejections still leave previously valid state unchanged

#### Scenario: fcis boundary rail detects core-type leakage in embedded runtime code
ID: no.std.functional.core.embedded.feedback.shell.native.adapters.fcis-boundary-rail-detects-core-type-leakage-in-embedded-runtime-code
- **WHEN** the FCIS shell-boundary rail inventories non-test runtime references in `src/modes/event_loop_runner/mod.rs`
- **THEN** it finds no `clankers_core` path usage at all in that file's non-test code
- **THEN** any reintroduced runtime core-type leakage fails the deterministic rail
