# Polyglot Agent Architecture

Clankers' dynamic-runtime architecture keeps configuration, authority, orchestration, execution, and receipts in separate layers:

| Layer | Owns | Must not own |
| --- | --- | --- |
| Nickel | Declarative agent/profile/tool/runtime contracts and policy exported before activation. | Runtime authority or imperative host effects. |
| UCAN | Runtime delegated authority: ability, resource, audience/session binding, expiry, revocation, and delegation limits. | Declarative policy shape or profile defaults. |
| Rust | Provider I/O, filesystem/process/network/credential/daemon/TUI authority, policy loading, UCAN verification, receipts, verification, rollback, and all side effects. | Unchecked dynamic-runtime authority bypasses. |
| Steel Scheme | Trusted, hot-reloadable orchestration: routing, scoring, planning, and typed requests to Rust host functions. | Ambient filesystem, shell, git, network, provider, credential, daemon, TUI, or native-tool access. |
| Wasm | Untrusted or generated tool execution behind explicit imports and bounded memory/fuel/time/input budgets. | Ambient host access or product claims that escape is impossible. |

Steel Scheme and Wasm are complementary. Steel may choose the next typed action, but Rust authorizes it. Wasm may execute untrusted tool code, but only through imports that Rust explicitly exposes and records.

## Dynamic-runtime seam

Every Steel or Wasm request that could cause a host-visible effect crosses a Rust-owned typed action envelope before execution. The host checks:

- Nickel-derived profile and policy allowance;
- UCAN-style runtime grant for sensitive actions;
- disabled action/tool state;
- session capabilities;
- runtime profile name and input budget;
- redaction class and safe receipt destination.

Denied requests produce deterministic receipts and perform no host effect.

## Steel wording

Describe Steel as a constrained embedded Steel Scheme interpreter or trusted orchestration runtime. Do not describe Steel as a VM, process, or OS-level sandbox unless there is a separate isolation proof. Its safety boundary is the Rust host-function surface plus profile/policy/UCAN checks.

## Wasm wording

Describe Wasm safety in terms of explicit imports, denied ambient filesystem/network access, bounded memory/fuel/time/input budgets, and receipt-backed runtime tests. Do not claim that Wasm escape is mathematically impossible as a product guarantee; the safety model depends on the Rust host imports and runtime configuration.

## Current deterministic rails

- `scripts/check-polyglot-agent-profile.rs` validates exported Nickel agent-profile contracts and receipt redaction policy.
- `scripts/check-polyglot-agent-boundaries.rs` rejects direct Steel/Nickel/Wasm runtime dependencies from generic engine/core/schema crates.
- `clankers-runtime::dynamic_runtime` contains deterministic Steel and Wasm fixtures for allowed actions, missing UCAN, policy denial, disabled/session-denied actions, ambient Steel access denial, missing Wasm imports, budget overflow, malformed schemas, and cross-layer receipt redaction.
