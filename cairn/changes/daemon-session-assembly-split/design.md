# Design: Split Daemon Session Assembly from Actor Loop

## Summary

Daemon sessions need a clearer boundary between assembly and runtime multiplexing. The actor loop should be boring: receive already assembled controller/tool/plugin inputs, poll channels, and broadcast events. Construction policy should live in socketless builders.

## Decisions

### 1. Assembly returns a single runtime bundle

Introduce a daemon session runtime bundle or extend `SessionBuilder` so spawn paths receive a prepared controller, event channels, tool rebuilder, hook pipeline, capability ceiling, and plugin projection handles. The actor loop consumes that bundle without constructing unrelated services inline.

### 2. Hook and plugin construction are builder-owned

Hook pipeline setup, plugin hook handler attachment, plugin summary projection, and tool-list refresh policy should move to named builder/projection modules. The actor loop may call refresh/drain helpers but should not own construction policy.

### 3. Spawn planning remains socketless-testable

Create, resume, keyed session recovery, and ephemeral child sessions should be planned in pure or socketless helpers. Tests can supply fake settings, paths, capabilities, and plugin managers without binding daemon sockets.

### 4. Actor loop parity is preserved by focused fixtures

Because daemon behavior is subtle, each extraction should keep a deterministic fixture around the moved seam: session capability merge, plugin tool-list sync, keyed-session recovery, and command/event drain behavior.

## Validation plan

- Socketless builder tests for create, resume, keyed, and ephemeral session assembly decisions.
- Focused daemon actor tests for tool-list/plugin summary refresh after the assembly split.
- Existing daemon/session recovery and attach parity tests that cover public behavior.
- Source-boundary rail that rejects hook/tool/plugin/capability construction in the actor loop outside assembly owners.
