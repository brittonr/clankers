# Change: Drain Agent Provider Coupling to Neutral DTOs

## Problem

`clankers-agent` still imports provider-owned request, usage, message, and stream types in turn policy, events, compaction, transcript, and tool-substrate paths. Some of those types are reexports from `clanker-message`, but importing them through `clankers-provider` keeps provider-native concerns visible inside the agent and makes model-port decoupling harder.

## Goals

- Replace provider reexport imports for messages, usage, and streaming deltas with `clanker-message` or engine/runtime-neutral DTOs.
- Keep provider-native `CompletionRequest` construction confined to a named model adapter boundary.
- Introduce or reuse neutral model request/stream DTOs for agent turn policy where practical.
- Reduce the `clankers-agent` concrete provider dependency budget or document the remaining adapter-only edge with a tighter convergence condition.

## Non-goals

- Do not change provider-native request body shaping or router policy.
- Do not remove all provider-backed model execution in this slice; the first goal is to confine it to the adapter seam.
- Do not rewrite compaction behavior beyond DTO import/projection cleanup unless required by the model-port seam.

## Proposed scope

Start with a mechanical but validated import migration from `clankers_provider::message`, `Usage`, and streaming reexports to `clanker-message`. Then narrow `CompletionRequest` and `Provider` references to model adapter files (`turn/execution.rs`, `turn/ports.rs`, or a new adapter module), with source rails preventing provider imports in reusable turn policy.

## Verification

Run focused agent turn/compaction/tool-substrate tests, an architecture import rail proving provider imports are adapter-only, `cargo check -p clankers-agent --tests`, lego dependency ownership rail, Cairn gates, and `git diff --check`.
