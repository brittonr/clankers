## Context

`src/modes/attach.rs` has grown to 2548 lines across three responsibilities:

1. **Local attach** — Unix socket connection to daemon, event processing, key handling, slash commands, reconnection (~1800 lines)
2. **Remote attach** — iroh QUIC connection, `QuicBiStream` adapter, QUIC framing, remote reconnection (~500 lines)
3. **Auto-daemon** — `AutoDaemonOptions`, `run_auto_daemon_attach`, `SessionGuard`, daemon lifecycle (~200 lines)

The remote and auto-daemon code share almost no internal state with local attach. They call into attach's event loop but are otherwise self-contained.

Separately, 5 clippy warnings have accumulated in test files from recent changes.

## Goals / Non-Goals

**Goals:**
- Split attach.rs into three files along existing responsibility boundaries
- Maintain all existing public APIs via re-exports
- Fix all clippy warnings
- Zero behavioral changes

**Non-Goals:**
- Refactoring the event processing logic within local attach
- Changing the QUIC protocol or framing
- Reducing the local attach file size further (1800 lines is acceptable for an event loop file)

## Decisions

### 1. Three files, not a directory module

Extract to `attach_remote.rs` and `auto_daemon.rs` as sibling files in `src/modes/`, not `src/modes/attach/mod.rs`. The attach module has no submodule hierarchy — flat files match the existing layout of `src/modes/`.

*Alternative: `attach/` directory with `mod.rs`, `remote.rs`, `auto_daemon.rs`.* Rejected because it changes more import paths and the existing modes directory is flat.

### 2. Re-export from attach.rs

`attach.rs` keeps `pub use attach_remote::*;` and `pub use auto_daemon::*;` so external callers (`main.rs`, `modes/mod.rs`) don't change their imports.

*Alternative: Update all call sites directly.* Rejected — more churn for zero benefit.

### 3. Visibility changes

Items currently `pub` stay `pub`. Items currently private that are needed cross-module become `pub(super)` or `pub(crate)` — whichever is narrowest.

### 4. Clippy fixes are separate commits

Clippy fixes go in their own commit before the decomposition, so the structural change is a pure move with no logic edits mixed in.

## Risks / Trade-offs

- [Risk: missed internal dependency] Functions in the extracted modules may reference private items in attach.rs. → Mitigation: `cargo check` after each extraction; widen visibility minimally.
- [Risk: git blame loss] Moving code to new files breaks `git blame`. → Mitigation: git detects renames when content change < ~20%. Keep extracted code identical.
- [Trade-off] Re-exports add a layer of indirection. Acceptable — callers already import from `modes::attach`.
