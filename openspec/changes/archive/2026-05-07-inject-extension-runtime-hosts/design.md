## Context

`clankers-runtime` is now the Rust embedding boundary for sessions, prompt assembly, tool catalogs, injected stores, and confirmations. Provider routing/auth and plugin/MCP runtime lifecycle remain powerful extension systems with their own side effects: process startup, socket/daemon contact, OAuth verifier persistence, token refresh writes, environment/header handling, and tool publication.

## Goals / Non-Goals

**Goals:**
- Make provider router construction, auth/credential stores, credential pools, and plugin/MCP runtime lifecycle explicit host-owned services for embedding.
- Preserve normal desktop Clankers behavior through adapters over the same contracts.
- Default embedded runtime construction to fail closed or disabled for side-effectful extensions.
- Ensure extension receipts/metadata are safe for replay/debug without credential leakage.

**Non-Goals:**
- Replacing `clanker-router` or existing provider backends.
- Redesigning OpenAI Codex, Anthropic, or OpenAI-compatible auth protocols.
- Replacing the plugin manifest/runtime model.
- Promoting all plugin/MCP/gateway surfaces to production-ready in this change.

## Decisions

### 1. Host-owned extension services

**Choice:** Add an extension-service boundary that lets hosts provide or disable router/provider, auth-store, credential-pool, and plugin/MCP runtime services explicitly.

**Rationale:** Embedders need to own process lifecycle, auth root paths, credential policies, and external tool publication. Hidden desktop defaults create surprising side effects and make embedding harder to audit.

**Alternative:** Let `clankers-runtime` call current desktop discovery paths directly. Rejected because it would reintroduce `~/.clankers`, daemon/router autostart, and plugin subprocess assumptions into the embedding boundary.

### 2. Desktop defaults are adapters, not core runtime behavior

**Choice:** Normal Clankers CLI/TUI/daemon behavior should be preserved by desktop adapter implementations that opt into existing router/auth/plugin defaults.

**Rationale:** Current users should not lose behavior, but embedding APIs should make those choices visible.

**Alternative:** Fork separate desktop and embedded provider/plugin implementations. Rejected because it would drift and weaken parity evidence.

### 3. Extension publication and extension execution are separate

**Choice:** Tool catalog publication may describe plugin/MCP/gateway tools only when a corresponding runtime service is enabled and healthy enough for publication; execution remains delegated to the host-owned runtime service.

**Rationale:** A catalog builder should not start subprocesses or OAuth flows merely to list tools.

**Alternative:** Eagerly start extension runtimes during catalog construction. Rejected because it violates default-safe embedding and makes read-only metadata queries side-effectful.

## Risks / Trade-offs

**Adapter sprawl** → Keep traits small and backed by parity fixtures against current desktop paths.

**Overclaiming production readiness** → Specs require fail-closed/default-disabled behavior and receipts; plugin/MCP production gaps remain separate changes.

**Credential leakage in debug metadata** → Require safe metadata fields only and negative tests for secrets/headers/request bodies.
