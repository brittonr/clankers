# Change: Split Daemon Session Assembly from Actor Loop

## Problem

`src/modes/daemon/agent_process.rs` still mixes daemon actor loop multiplexing with session runtime assembly, hook pipeline construction, capability gate creation, tool rebuilding, plugin summary projection, and keyed/ephemeral session spawn planning. Some session builder seams exist, but the actor module remains a broad policy owner.

## Goals

- Keep `agent_process.rs` focused on actor signals, command/event multiplexing, and session lifecycle plumbing.
- Move hook pipeline construction, capability gate assembly, tool rebuilder construction, plugin summary/tool-list projection, and spawn inputs into socketless builders/adapters.
- Make create/resume/keyed/ephemeral session assembly testable without binding Unix sockets or requiring a running actor registry.
- Preserve daemon, local attach, remote attach, subagent, delegate, Matrix/keyed-session, and plugin hot-reload behavior.

## Non-goals

- Do not replace the actor system or daemon protocol.
- Do not change public daemon commands or session IDs.
- Do not recursively make child factories load plugins; keep the current fallback policy unless a separate change updates it.

## Proposed scope

Extract a daemon session assembly layer that produces `SessionController`, tool rebuilder, hook pipeline, capability ceiling, plugin projection, and spawn plan inputs before the actor loop begins. The actor loop should receive assembled runtime inputs and only poll commands, signals, confirmations, plugin events, and controller events.

## Verification

Validation should include socketless session assembly fixtures for create/resume/keyed/ephemeral sessions, daemon actor loop parity tests for tool list/plugin updates, existing session recovery tests, architecture rails, Cairn gates, and `git diff --check`.
