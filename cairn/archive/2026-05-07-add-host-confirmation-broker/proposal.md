## Why

Embedding applications need to route confirmations through their own UI, permission, audit, or policy systems. Confirmation requests should not be tied to TUI modals, daemon events, slash commands, or raw terminal input. A host confirmation broker makes dangerous actions embeddable without bypassing Clankers policy.

## What Changes

- Define a host-facing confirmation broker interface for tool/action confirmations.
- Route confirmation requests and decisions through typed ids, safe summaries, expiry/cancellation state, and audit metadata.
- Adapt TUI, daemon/attach, MCP/ACP, and embedding hosts over the same confirmation substrate.

## Scope

In scope: broker interface, request/decision types, safe metadata, default deny/unavailable behavior, timeout/cancel handling, and parity with existing confirmation flows.

Out of scope: changing which tools require confirmation, implementing app-specific UI widgets, or allowing MCP/ACP/private adapters to bypass confirmation requirements.

## Verification

Validate with fake broker tests, denied/unavailable/timeout paths, existing TUI/daemon confirmation parity tests, and negative tests proving actions do not execute before approval.
