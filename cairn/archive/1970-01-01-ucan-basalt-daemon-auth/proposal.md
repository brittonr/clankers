# Proposal: UCAN + Basalt Daemon Auth

## Why

Clankers already has two auth-related stacks, but the daemon/remote access path still trusts the workspace-local `clanker-auth` token format by default. `crates/clankers-ucan` re-exports that custom verifier, while the OnixResearch `ucan` crate is only feature-gated behind `external-ucan` / `runtime-admission`. Basalt is present in the workspace, but today it is used for Steel/runtime contract receipts rather than daemon credential verification.

That split makes the roadmap item "UCAN Auth" ambiguous: remote daemon auth is capability-shaped, but it is not yet the public UCAN + Basalt policy boundary we want to rely on for delegated access across QUIC, Matrix, chat/RPC, session attach, and tool execution.

## What Changes

Switch remote daemon auth to canonical public UCAN tokens from OnixResearch `ucan`, with Basalt as the mandatory policy/receipt layer for remote admission decisions.

- Public UCAN compact tokens and proof chains become the default daemon credential format.
- Basalt policy enforcement becomes part of session create, attach, RPC/chat, Matrix, and per-tool authorization for remote sessions.
- The existing `clanker-auth` token path is removed from default daemon trust and kept only as an explicit migration/import compatibility path if needed.
- `clankers-ucan` becomes the Clankers vocabulary/adapter crate over public UCAN + Basalt rather than a re-export of the custom verifier.
- Operator-visible receipts and diagnostics expose policy hashes, proof references, resources, abilities, and deny reasons without leaking raw tokens, signing material, prompts, provider payloads, or tool inputs beyond approved metadata.

## Impact

- **Files**: `crates/clankers-ucan/`, `crates/clanker-auth/` usage sites, `src/modes/daemon/{session_store,quic_bridge,handlers,agent_process}.rs`, `src/capability_gate.rs`, token CLI/config/docs, `Cargo.toml`, `Cargo.lock`, `crate-hashes.json`, Nix source pins.
- **Testing**: deterministic UCAN issue/verify/delegate fixtures, Basalt allow/deny policy fixtures, daemon QUIC control/attach/RPC auth tests, Matrix keyed-session auth tests, tool-gate mapping tests, receipt-redaction tests, dependency/source checks, Cairn validation/gates.
