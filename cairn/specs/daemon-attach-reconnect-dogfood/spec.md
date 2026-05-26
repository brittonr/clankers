# Daemon Attach Reconnect Dogfood Specification

## Purpose

Defines the focused local daemon attach/reconnect dogfood rail. The rail proves operator-visible local socket attach recovery without live credentials, and remains opt-in until promoted into full readiness.

## Requirements

### Requirement: Local reconnect preserves session view [r[daemon-attach-reconnect-dogfood.local-reconnect]]

The daemon attach reconnect dogfood rail MUST prove that detaching and reattaching a local TUI client returns to the intended daemon session rather than forking or losing visible history.

#### Scenario: Local reconnect restores session view

- GIVEN a local daemon session exists and has visible assistant history produced through the real TUI attach path
- WHEN the first attached TUI detaches and a second TUI attaches to the same daemon session id
- THEN the second attached UI shows the expected history sentinel
- AND the daemon session count before and after reattach remains one
- AND the receipt records `replayed_history_visible: true` and `session_not_forked: true`

### Requirement: Attach parity state resets before post-reconnect events [r[daemon-attach-reconnect-dogfood.parity-reset]]

The attach reconnect implementation MUST reset local attach parity suppression state before processing new daemon events after reconnect.

#### Scenario: Stale suppression budget does not hide a legitimate acknowledgement

- GIVEN an attached client has local slash/action parity suppression budget before reconnect
- WHEN the client reconnects and receives a new daemon system acknowledgement
- THEN the acknowledgement is visible instead of hidden by stale suppression state
- AND a deterministic regression test covers this ordering in the local reconnect helper

### Requirement: Provider behavior is deterministic [r[daemon-attach-reconnect-dogfood.deterministic-provider]]

The dogfood rail MUST avoid live model credentials and network-dependent provider behavior.

#### Scenario: Local provider stub drives the rail

- GIVEN the dogfood rail runs in local developer or CI-like conditions
- WHEN it needs model output for replay evidence
- THEN it uses a local deterministic provider stub rather than real model credentials
- AND the receipt records `deterministic_provider: true` and `provider_requests > 0`

### Requirement: Cleanup-aware receipt is emitted [r[daemon-attach-reconnect-dogfood.cleanup-receipt]]

The dogfood rail MUST emit bounded artifacts and cleanup evidence under `target/dogfood/daemon-attach-reconnect-*`.

#### Scenario: Daemon dogfood cleanup is verified

- GIVEN the dogfood rail completes successfully
- WHEN it writes `receipt.json`
- THEN the receipt uses schema `clankers.daemon_attach_reconnect_dogfood.receipt.v1`
- AND it records daemon/session identifiers, screen artifacts, replay assertions, deterministic provider assertions, and cleanup status
- AND it records `daemon_cleaned_up: true`

### Requirement: Harness surface is opt-in [r[daemon-attach-reconnect-dogfood.opt-in-harness]]

The daemon attach reconnect dogfood rail MUST be discoverable through the harness dogfood profile without becoming part of normal full readiness in this slice.

#### Scenario: Focused harness selector runs the rail

- GIVEN an operator needs focused local daemon attach/reconnect evidence
- WHEN they run `./scripts/test-harness.sh dogfood daemon-attach-reconnect`
- THEN the harness runs `./scripts/check-daemon-attach-reconnect-dogfood.rs`
- AND `./scripts/test-harness.sh full` is unchanged by this slice
