# Proposal: Polyglot Agent Architecture

## Summary

Define Clankers' long-term polyglot agent architecture as an explicit division of labor across Nickel, Rust, Steel Scheme, Wasm, and UCAN. The goal is not to add another runtime for its own sake; it is to make each layer own the authority it is best suited to own:

- Nickel declares validated agent/persona/prompt/tool/policy contracts.
- Rust owns the cognitive engine shell: async I/O, provider routing, memory/session state, enforcement, receipts, verification, and rollback.
- Steel Scheme owns trusted, hot-reloadable orchestration logic for reasoning loops and routing.
- Wasm owns untrusted or third-party tool execution behind explicit capability imports.
- UCAN grants runtime authority for session/script/tool-specific actions.

## Motivation

Current agent frameworks often blur configuration, orchestration, tool execution, and authority. That makes prompt/schema drift easy, tool execution unsafe, and runtime mutation hard to audit. Clankers already has native seams for an embeddable engine, Wasm plugin/runtime work, Steel Scheme runtime planning, Steel self-mutation policy, Nickel policy export rails, and UCAN-oriented effect permissions. This change records the architecture that composes those pieces into a coherent next-generation agent kit.

## Problem

Without a first-class architecture contract, future work can drift into unsafe or brittle shapes:

- Steel could accidentally become ambient host authority instead of trusted orchestration over typed host functions.
- Wasm could be documented as a magic sandbox instead of capability-limited execution controlled by Rust imports and runtime budgets.
- Nickel prompt/tool/schema contracts could remain ad hoc files instead of boot-time validated product inputs.
- Rust engine and shell crates could duplicate policy or bypass declarative contracts.
- UCAN runtime grants could be omitted from dynamic agent actions because Nickel already says something is allowed in principle.

## Proposed change

Add a Cairn package that defines the polyglot agent architecture requirements and implementation tasks. This package will not implement the whole stack immediately. It will establish the cross-layer contracts, verification rails, and dependency boundaries for follow-up implementation slices.

## Non-goals

- Do not claim Steel is an OS/process sandbox.
- Do not claim Wasm makes escape mathematically impossible without host-runtime proof.
- Do not grant Steel raw filesystem, process, git, network, provider, credential, daemon, TUI, or native-tool authority.
- Do not replace the existing Rust engine/provider/tool-host architecture in one broad rewrite.
- Do not implement live self-mutation beyond the already scoped Steel self-mutation policy rails.
