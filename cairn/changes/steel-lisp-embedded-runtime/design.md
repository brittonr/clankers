# Design: Steel Lisp Embedded Runtime

## Context

Clankers already supports built-in tools, Extism WASM plugins, stdio plugin tools, daemon tool rebuilding, UCAN-style capability gates, and an embeddable engine direction. Steel Lisp should compose with those seams rather than becoming another product-shell path. The durable boundary is: Steel evaluates local Scheme code inside a Rust-owned runtime wrapper, while Clankers owns all host effects through explicit ports, tool calls, and receipts.

## Decisions

### 1. Steel runtime is a constrained in-process adapter

**Choice:** Add a focused Steel runtime wrapper that embeds the `steel-lang` interpreter behind Clankers-owned request/response DTOs. The wrapper accepts source text or a reviewed script reference, a bounded execution profile, an allowlist of host functions, and a capability context. It returns structured output and a deterministic receipt.

**Rationale:** Steel is useful because it is embeddable Rust Scheme. Wrapping it behind Clankers DTOs keeps the public contract stable if the upstream Steel API changes and avoids leaking interpreter internals into daemon, TUI, or provider code.

### 2. Host effects are explicit functions, never ambient authority

**Choice:** Steel code may call only host functions registered by Clankers for that evaluation. Host functions must map to existing typed tool/effect seams and must pass disabled-tool and capability checks before execution. Filesystem, process, network, provider/router, credential, daemon session, and TUI effects are unavailable unless a future change adds a named host function with its own requirement IDs and tests.

**Rationale:** An embedded Lisp can otherwise become a universal escape hatch around Clankers policy. This keeps Steel as a programmable frontend to approved capabilities, not a second privileged runtime.

### 3. CLI and tool surfaces use deterministic receipts

**Choice:** Provide `clankers steel status`, `clankers steel eval`, and `clankers steel run` as the first operator surfaces. An optional `steel_eval` agent tool may be registered only when the same runtime wrapper, resource controls, redaction policy, and capability checks are used. Results include stable fields for runtime version, profile, allowed host functions, output classification, issue codes, and redacted diagnostics.

**Rationale:** CLI-first status/eval/run makes the feature testable before broad agent exposure. Shared receipts make daemon, standalone, and docs evidence comparable.

### 4. Resource controls fail closed

**Choice:** The runtime must enforce bounded source size, output size, host-call count, recursion/step or fuel limits when available, and wall-clock timeout at the shell boundary. Limit failures return typed denial/failure receipts and must not retry with a less restricted profile.

**Rationale:** Embedded scripting inside an agent can otherwise hang turns or flood transcripts. Even if some limits are enforced outside Steel, the Clankers API must expose them as named profiles and test them.

### 5. Verification covers positive and negative behavior

**Choice:** Implementation must include: pure/runtime unit tests for deterministic Lisp evaluation; host-function tests with fake approved functions; negative tests for unknown/denied host functions and ambient effect attempts; CLI receipt tests; and daemon/tool-inventory parity if agent tool exposure is enabled.

**Rationale:** The risk is not whether arithmetic evaluates, but whether the feature preserves Clankers capability, redaction, and daemon parity contracts.

## Risks / Trade-offs

- **Upstream API drift:** Steel APIs may change. Keep the direct dependency isolated in the runtime wrapper and pin/update Cargo/Nix together.
- **Security expectations:** This is not a hardened VM sandbox. The first implementation must document non-claims and rely on deny-by-default host effects, resource limits, and Rust process isolation only where explicitly added.
- **Prompt/tool abuse:** Agent-visible Steel evaluation can become arbitrary computation. Start CLI-first or gate the tool behind explicit capability/toolset policy.
- **Determinism:** Interpreter diagnostics or version strings can drift. Receipts should separate stable issue codes from human diagnostics and include version/profile metadata.
