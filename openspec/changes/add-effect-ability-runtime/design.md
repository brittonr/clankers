## Context

Clankers already has explicit tool catalogs and embeddable runtime services. This change tightens those seams into a uniform typed effect model inspired by algebraic effects: code requests an effect, and a host-installed handler decides behavior.

## Goals / Non-Goals

**Goals:**
- Classify side effects under typed ability interfaces.
- Make handlers swappable for real execution, denial, replay, and simulation.
- Prove fail-closed behavior for absent handlers.
- Sync required safe artifacts by content hash for remote/subagent execution.

**Non-Goals:**
- Implement a language-level algebraic effects system in Rust.
- Remove existing tool names or user-facing capability packs in one step.
- Auto-sync credentials or secret payloads between machines.

## Decision 1: Effects are typed request envelopes

**Choice:** Tool/runtime operations emit typed effect request envelopes with effect class, input schema, artifact dependencies, redaction class, and correlation ID.

**Rationale:** This gives a uniform place for policy, replay, simulation, and receipts.

**Alternative:** Keep per-tool bespoke permission checks only. Rejected because it makes cross-cutting guarantees hard to verify.

## Decision 2: Handlers are host-owned and fail closed by default

**Choice:** If a handler for an effect class is absent, denied, or lacks a required dependency, the effect fails before side effects occur.

**Rationale:** Embedding and daemon safety require absence to be safe.

## Decision 3: Remote sync transfers safe artifacts, not secrets

**Choice:** Remote/subagent dependency sync fetches content-addressed skills, prompts, tool schemas, manifests, and policy metadata. Credential values, env values, and raw provider secrets are never synced.

**Rationale:** Reproducibility must not become secret exfiltration.

## Risks / Trade-offs

**Abstraction churn** → Start with wrappers around existing tool dispatch and capability packs.

**False safety claims** → Add sentinels for filesystem, process, socket, browser, and secret access in deny/simulate tests.

**Remote version skew** → Include artifact schema version and unsupported-artifact errors.

## Validation Plan

- Matrix tests for every effect class: absent handler, deny, allow, simulate, replay.
- Handler receipts prove redaction and correlation IDs.
- Subagent/remote protocol tests request missing artifacts by hash and fail closed for missing/unsupported/secret dependencies.
- Existing catalog capability-pack tests continue to pass through adapter mapping.
