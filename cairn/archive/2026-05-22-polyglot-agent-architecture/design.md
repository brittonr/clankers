# Design: Polyglot Agent Architecture

## Overview

The architecture turns Clankers into a polyglot agent kit without collapsing authority into the most dynamic language. The key rule is:

```text
Nickel = declared configuration and policy contracts
UCAN   = runtime delegated authority
Rust   = enforcement, I/O, receipts, verification, rollback
Steel  = trusted orchestration/request logic
Wasm   = untrusted or third-party tool execution boundary
```

Steel and Wasm are complementary. Steel is the internal frontal-lobe layer for trusted, hot-reloadable reasoning and routing logic. Wasm is the hands layer for isolated tool plugins and untrusted generated code. Rust remains the nervous system and enforcement point for both.

## Layer contracts

### Nickel: agent/persona/prompt/tool policy

Nickel-owned contracts should describe:

- agent identity/persona metadata;
- system prompt templates and required variables;
- model/provider selection profiles;
- tool manifests and JSON schemas;
- runtime profiles and budgets;
- declarative permission/policy defaults;
- schema compatibility and migration metadata.

Nickel validation happens before agent boot or runtime profile activation. Runtime code consumes exported typed data or generated fixtures; hot-path enforcement does not depend on ad hoc stringly configuration.

### Rust: engine, I/O, and authority

Rust owns:

- embeddable engine state/effects;
- provider routing and streaming;
- memory/session persistence;
- tool dispatch and host-function registration;
- UCAN verification/adaptation;
- policy consumption from exported Nickel data;
- deterministic receipts;
- verification/rollback;
- process, filesystem, network, credential, daemon, and TUI authority.

Rust adapters may call Steel or Wasm, but Rust decides which imports/functions exist, which capabilities are granted, and which receipts are emitted.

### Steel Scheme: trusted orchestration

Steel scripts may own workflow logic such as ReAct-like loops, tree/graph search policy, routing decisions, scoring, memory update plans, and mutation requests. Steel scripts must interact with the host only through typed registered host functions. They do not get ambient authority or a claim of full sandbox isolation.

Steel orchestration is allowed to be hot-reloadable because the host-function surface is stable and capability-gated. This lets agent behavior change without recompiling Rust while preserving Rust-owned enforcement.

### Wasm: tool and generated-code sandbox boundary

Wasm plugins/tools must receive only explicit imports and host-provided data. Wasm execution should be bounded by memory/fuel/time budgets, no ambient filesystem/network access, and manifest-declared tool schemas. For generated code execution, the host should create an ephemeral Wasm execution context, provide only allowed imports, collect structured output, and destroy the context.

Wasm documentation must avoid overclaiming. Safety comes from the runtime configuration, denied imports, capability model, budgets, and tests—not from the word "Wasm" alone.

### UCAN: runtime grants

Nickel defines what may be allowed in principle. UCAN defines what this session/script/tool invocation is allowed to do now. Sensitive actions require both:

1. Nickel policy allows target/verb/profile.
2. UCAN grant matches ability/resource/audience/expiry/delegation/revocation constraints.

Receipts include safe UCAN metadata only, never compact tokens, bearer credentials, private keys, or raw proofs.

## Architecture boundaries

- Generic SDK/plugin/tool types must not depend on live Nickel evaluation.
- Engine/core crates must not gain direct Steel interpreter dependencies.
- Steel runtime wrapper owns Steel interpreter integration.
- Wasm plugin runtime owns Wasm loading/invocation; plugins do not receive host authority except declared imports.
- Rust host-function bridges are the only path from Steel/Wasm into host side effects.
- Prompt/persona/tool config validation belongs to Nickel export/check rails and typed Rust DTOs.

## Verification strategy

The first implementation should add architecture rails and fixtures before broad feature work:

- validate Nickel agent config/persona/tool schema fixtures;
- check public crate dependency boundaries for Steel/Wasm/Nickel leakage;
- test a Steel orchestration script that requests typed host actions only;
- test a Wasm tool/plugin with no ambient filesystem/network imports;
- test UCAN-denied and Nickel-denied runtime actions;
- emit deterministic receipts for allowed/denied orchestration and tool execution paths;
- add wording checks or docs tests preventing sandbox overclaims.

## Risks

- Treating Steel as an authority layer would undermine the design.
- Treating Wasm as magic isolation would create a false safety claim.
- Combining Nickel and UCAN into one concept would lose the distinction between declared policy and runtime delegation.
- Over-broad first implementation could destabilize existing engine/provider/tool behavior.

## Migration approach

Start with contracts and rails, then drain narrow slices:

1. Agent configuration/persona/tool schema Nickel contracts.
2. Rust DTO loader/checker for exported agent config.
3. Steel orchestration host-function bridge fixture.
4. Wasm capability import/budget fixture.
5. Engine/tool-host integration tests proving the layer split.
6. Documentation and status output using precise safety language.
