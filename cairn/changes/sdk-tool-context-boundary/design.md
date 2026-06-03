# Design: Drain Legacy ToolContext Into Neutral Tool Services

## Summary

`clankers-tool-host` is the SDK-friendly contract; `ToolContext` is a legacy shell adapter. This change makes that distinction enforceable by migrating representative tools and preventing new coupling through the old context.

## Current coupling points

- `crates/clankers-agent/src/tool.rs::ToolContext` stores concrete `clankers_db::Db`, search index, `clankers_hooks::HookPipeline`, `AgentEvent` sender, and cancellation token.
- Built-in root tools can request those concrete services through the context.
- `ControllerToolServices::from_concrete` already builds neutral services, but legacy tools still receive concrete fields through `LegacyToolRunner`.

## Decisions

### 1. Legacy context is compatibility-only

The old context stays available while tools migrate, but new reusable tool behavior should use neutral service traits from `clankers-tool-host`.

### 2. Migrate by service family

Storage/search, hooks, progress/events, capability, and cancellation should each have at least one representative production or fixture path that proves the neutral service works.

### 3. Missing services fail closed

A tool that requires a service absent from the neutral bundle must return a typed safe failure rather than constructing a desktop default.

## Validation plan

- Tool migration inventory with service families and current context users.
- Neutral-native tool tests for success and missing-service cases.
- Legacy parity tests proving unchanged behavior for unmigrated tools.
- Source rails over `clankers-tool-host` and migrated tools.
