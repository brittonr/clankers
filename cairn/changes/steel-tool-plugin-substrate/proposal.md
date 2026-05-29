# Proposal: Steel Tool, Plugin, and Subagent Substrate

## Problem

Clankers currently uses Steel only as a reviewed `steel.host.plan_turn` turn-planning seam. Tool execution still falls through Rust-native dispatch paths: built-in tools call `Tool::execute`, WASM plugins call the plugin manager, and stdio plugins use their supervisor/tool-call channel directly. That split makes Steel an advisor rather than the orchestration substrate for the work the agent actually performs.

The product direction is to make Steel the substrate that routes tool and plugin invocations while preserving the existing safety boundary: Steel may propose typed host actions, but Rust remains the only authority that validates policy, checks capabilities, executes effects, handles cancellation, and records receipts.

## Proposed Change

Add a Steel-mediated tool/plugin/subagent substrate that covers Rust built-in tools, WASM/Extism plugins, stdio plugins, and subagent/delegate agent execution behind one typed invocation contract. The substrate introduces reviewed Steel host functions for catalog discovery and call planning, returns versioned action envelopes, and routes every selected action back through Rust-owned authorization and executor adapters.

The change is intentionally staged:

1. Define the substrate DTOs, policy/profile shape, receipts, and Rust-owned adapter seam.
2. Route Rust built-in tool calls through the substrate in comparison mode, then default mode.
3. Route WASM/Extism plugin tool calls through the same substrate.
4. Route stdio plugin tool calls through the same substrate while preserving async progress, timeout, cancellation, and supervisor lifecycle semantics.
5. Route `subagent` and `delegate_task` calls through the same substrate while preserving actor/subprocess spawning, panel events, watchdogs, and cancellation semantics.
6. Make Steel-mediated dispatch the default for tool/plugin/subagent calls once fixture, runtime, and dogfood evidence prove parity and fail-closed behavior.

## Impact

- Agent turn execution gains one Steel-mediated dispatch port instead of separate built-in/plugin/subagent execution branches owning policy decisions.
- Existing `Tool`, `PluginTool`, `PluginHostFacade`, WASM manager calls, stdio runtime calls, `SubagentTool`, and `DelegateTool` remain Rust executors behind adapters during migration.
- Steel receives only bounded, redacted catalog/call metadata and returns typed envelopes; it never gets ambient filesystem, process, network, provider, daemon, TUI, or plugin-manager authority.
- Every Steel-mediated call produces deterministic receipt material naming executor kind, policy/profile, input/output hashes, authorization status, fallback mode, child-agent/subagent status when applicable, and redaction class.
- Direct dispatch remains available only as an explicit comparison/fallback/kill-switch path until the default rollout is proven.

## Non-goals

- Do not embed arbitrary Rust, WASM, or agent execution inside the Steel interpreter.
- Do not bypass existing capability gates, disabled-tool policy, plugin launch policy, stdio sandboxing, hook/cancellation behavior, or tool output truncation.
- Do not grant Steel raw prompts, subagent task bodies/transcripts, credentials, provider payloads, tool bodies, plugin stdout/stderr streams, or unbounded result data.
- Do not remove Rust-native direct dispatch until the final rollout task proves parity and operator opt-out behavior.
