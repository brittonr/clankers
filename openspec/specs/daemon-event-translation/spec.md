# Daemon Event Translation Specification

## Purpose

Defines the reusable daemon event translation contract for streaming, tool/session events, attach replay, and app-edge event handling.

## Requirements

### Requirement: Daemon event translation kit preserves replay compatibility [r[daemon-event-translation.daemon-event-translation-kit]]

The system MUST define `daemon-event-translation-kit` as a composable Clankers brick with explicit ownership boundaries, deterministic fixtures, and safe evidence.

#### Scenario: Brick boundary is explicit [r[daemon-event-translation.daemon-event-translation-kit.boundary]]

- GIVEN a product or contributor adopts the `daemon-event-translation-kit` brick
- WHEN the brick is documented, instantiated, or validated
- THEN the contract MUST name `AgentEvent` to `DaemonEvent`, `DaemonEvent` to `TuiEvent`, and stored-message replay to `TuiEvent` translation as reusable behavior
- THEN session metadata, system messages, confirmation dialogs, plugin widgets, subagent panes, and replay control markers MUST remain attach/app-edge behavior unless a future design explicitly promotes them
- THEN the brick MUST NOT silently depend on ambient credentials, daemon sessions, live TUI state, provider discovery, plugin supervision, Matrix, iroh, or global singleton runtime state unless the design explicitly labels that path as app-edge

#### Scenario: Streaming/replay translation has executable evidence [r[daemon-event-translation.daemon-event-translation-kit.streaming-replay]]

- GIVEN daemon streaming or replay translation changes
- WHEN the focused verification for `daemon-event-translation-kit` runs
- THEN it MUST exercise at least one positive streaming `DaemonEvent` to `TuiEvent` path
- THEN it MUST exercise at least one positive user or history replay path that preserves deterministic timestamps or block reconstruction state
- THEN evidence MUST be safe to commit or summarize without raw prompts, credentials, authorization headers, OAuth tokens, provider payloads, hidden context, raw tool arguments, or secret environment values

#### Scenario: App-edge-only events fail closed at the shared translator [r[daemon-event-translation.daemon-event-translation-kit.app-edge]]

- GIVEN a `DaemonEvent` is owned by attach/client shell behavior rather than the shared TUI event stream
- WHEN `daemon_event_to_tui_event` receives that event
- THEN the shared translator MUST return `None` instead of inventing a lossy `TuiEvent`
- THEN the attach/client shell MUST own any follow-up state mutation or diagnostic handling
- THEN focused evidence MUST include at least one app-edge or replay-metadata event that does not translate into a `TuiEvent`

#### Scenario: Brick drift is diagnosable [r[daemon-event-translation.daemon-event-translation-kit.drift]]

- GIVEN source code, docs, fixtures, policy, or generated inventories drift apart
- WHEN `scripts/check-daemon-event-translation-kit.rs` runs
- THEN it MUST fail with a diagnostic that names the stale artifact and the expected owner of the update
- THEN intentional contract changes MUST require updating tests, docs, and OpenSpec evidence together
