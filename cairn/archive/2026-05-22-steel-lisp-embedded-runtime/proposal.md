# Proposal: Steel Lisp Embedded Runtime

## Why

Clankers has Rust-native plugin, tool, and embeddable-engine seams, but local automation that wants a small programmable policy layer still has to choose between compiled Rust plugins, external stdio processes, or shell scripts. That leaves no first-class in-process Lisp/Scheme surface for users to write small, inspectable extensions that can call approved Clankers host functions without taking a dependency on the full product shell.

Steel is an embeddable Scheme/Lisp implemented in Rust (`steel-lang`). Clankers should evaluate it as a constrained embedded runtime, not as an unbounded scripting backdoor.

## What Changes

Add a native Cairn package for integrating Steel Lisp into Clankers as a capability-gated embedded scripting runtime. The implementation should add a narrow Rust runtime wrapper, CLI/tool surfaces for deterministic evaluation, explicit host-function registration, safe receipts, resource controls, and tests that prove both allowed scripts and denied effects.

This package is implementation-directed planning: it defines the requirements and tasks for the implementation but does not implement Steel in this commit.

## Impact

- **Files**: expected implementation under a focused runtime crate or module such as `crates/clankers-steel/`, root CLI dispatch, tool registration/daemon adapter seams, policy/receipt fixtures, docs, and focused tests.
- **APIs**: adds explicit Steel-facing public surfaces only after they are capability-gated: CLI eval/run/status, optional built-in tool exposure, and host-function registration APIs for approved Clankers capabilities.
- **Security**: no implicit filesystem, process, network, credential, provider, daemon, or TUI access from Steel scripts; every host effect must pass the same Clankers capability and disabled-tool gates as native tools.
- **Testing**: deterministic positive evaluation, host-function invocation, denied-effect, resource-limit, receipt-redaction, and daemon/tool-inventory parity checks.
