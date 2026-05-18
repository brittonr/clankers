## Context

The green embedded SDK surface now has concrete examples for minimal execution, tools, and product-owned provider conversion. Existing OpenSpecs already require host-owned runtime stores and in-memory session replay parity, but product embedders still lack a small executable recipe that shows what to persist and how to restore context without adopting Clankers' desktop storage.

## Goals / Non-Goals

**Goals:**

- Prove a product can own session persistence with recipe-local DTOs and store implementation.
- Demonstrate restore/resume context using `EngineMessage` history and `EngineModelRequest` observations.
- Keep storage/session ownership outside green SDK crates unless later evidence justifies promotion.
- Add acceptance coverage so the recipe cannot drift or accidentally import shell/runtime storage crates.

**Non-Goals:**

- Stabilizing a public `SessionStore` trait in `clankers-adapters`.
- Replacing `clankers-session`, JSONL files, daemon sessions, or desktop/TUI restore paths.
- Adding database migrations, durable file formats, sync/conflict resolution, or multi-device storage semantics.
- Promising public semver compatibility for recipe-local product DTOs.

## Decisions

### 1. Recipe-local product DTOs first

**Choice:** Implement the first storage/session proof as recipe-local DTOs such as `ProductSession`, `ProductMessage`, and optional `ProductTurnReceipt`.

**Rationale:** This keeps the generic SDK from prematurely owning product storage schema. The recipe can teach the pattern while allowing products to map to SQLite, Postgres, S3, CRDTs, or in-memory tests.

**Alternative:** Add a reusable `SessionStore` trait to `clankers-adapters` immediately. Rejected because one recipe is not enough evidence for a stable trait shape.

### 2. Restore by converting product transcript to engine history

**Choice:** The recipe should reconstruct `Vec<EngineMessage>` or the existing engine prompt submission history from product-owned messages, then assert the follow-up model request includes restored context.

**Rationale:** `EngineMessage` is already a green SDK data type. The conversion seam is the product-owned boundary; the engine should not know about DB rows, JSONL entries, or daemon session metadata.

**Alternative:** Reuse Clankers JSONL/session types. Rejected because it would imply desktop persistence is the embedding API.

### 3. Acceptance through the existing embedded SDK rail

**Choice:** Add the recipe to `scripts/check-embedded-agent-sdk.sh` and extend dependency-denylist checks as needed.

**Rationale:** The user-facing readiness claim is already one command. Storage/session should become part of that same claim rather than a separate informal example.

### 4. Fail closed for missing sessions

**Choice:** Missing session restore should return a typed/product-owned error and assert no hidden replacement session was created.

**Rationale:** Silent session creation is dangerous for product embedding because it hides data loss, breaks auditability, and can mask wrong tenant/session routing.

## Risks / Trade-offs

**[Recipe becomes de facto API]** → Keep DTOs inside the example and document them as a pattern, not a promised crate-level API.

**[Context assertions too weak]** → Assert recorded `EngineModelRequest` messages contain prior user/assistant content and follow-up content in order, not merely that the run succeeds.

**[Dependency boundary regression]** → Add or reuse denylist checks so the recipe cannot import `clankers-db`, `clankers-session`, daemon/TUI/provider/router, or OAuth machinery.

**[Overly broad implementation]** → Keep the first slice in-memory and deterministic. Defer durable DB/file formats to product-specific follow-up work.
