## Why

Clankers already has reusable controller, agent, engine, provider, and session crates, but embedding another application still means discovering daemon/TUI/CLI-shaped wiring. Host applications need a stable Rust API that creates sessions, submits prompts, streams typed events, and controls common session operations without driving a subprocess, daemon socket, ACP/MCP bridge, or TUI state.

## What Changes

- Add an embeddable runtime facade such as `clankers-runtime` or `clankers-embed`.
- Expose `RuntimeBuilder`, `SessionHandle`, host-friendly prompt input, session-control methods, and typed event streams.
- Treat daemon, TUI, ACP, MCP, Matrix, and CLI as adapters over the same runtime API rather than privileged internal entrypoints.

## Scope

In scope: API contract, lifecycle ownership, typed event stream, initial in-memory/embed-friendly defaults, and adapter parity rails.

Out of scope: rewriting provider backends, replacing the daemon protocol, or guaranteeing a stable C/FFI/web API in the first slice.

## Verification

Validate with runtime crate API tests, adapter parity tests that CLI/daemon still use the same session lifecycle semantics, and boundary rails that the public embed API does not expose TUI or daemon transport types.
