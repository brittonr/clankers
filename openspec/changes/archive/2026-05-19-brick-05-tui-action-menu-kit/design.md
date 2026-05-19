## Context

TUI actions and leader menu contribution types are already separated into shared TUI types, but extension-safe composition lacks a dedicated contract and inventory rail.

Evidence anchors: crates/clanker-tui-types/src/actions.rs; crates/clanker-tui-types/src/menu.rs.

## Goals / Non-Goals

**Goals:**
- Make `tui-action-menu-kit` copyable, inspectable, and testable as a small brick.
- Add deterministic positive and negative evidence for the brick boundary.
- Keep the contract narrow enough to drain independently.

**Non-Goals:**
- Do not redesign unrelated Clankers runtime, provider, daemon, plugin, or TUI surfaces.
- Do not move product-owned I/O or app-edge shell behavior into green SDK crates.
- Do not claim semver/stability guarantees beyond the evidence this change adds.

## Decisions

### 1. Treat this as a product-facing brick, not a broad refactor

**Choice:** Implement the smallest recipe, manifest, policy, generated inventory, or receipt validator that proves `tui-action-menu-kit`.

**Rationale:** Brick value comes from reuse and verification, not from moving large surfaces.

**Alternative:** Extract a generic crate immediately. Rejected because real dogfood and fixture evidence should precede public API expansion.

### 2. Prefer deterministic evidence over prose-only docs

**Choice:** The implementation must produce an executable example, focused test, policy checker, generated inventory, or receipt fixture.

**Rationale:** Lego-like composition needs drift detection and safe proof that downstream products can copy the pattern.

### 3. Preserve ownership boundaries

**Choice:** Any shell/runtime integration remains behind app-edge adapters or product-owned traits.

**Rationale:** The embedded lego direction depends on functional-core / imperative-shell separation and avoiding ambient global services.

## Risks / Trade-offs

**Over-abstraction risk** → Start with fixtures and recipes before extracting new public crates.

**Boundary leakage risk** → Add dependency or source-inventory checks when this brick touches green SDK crates.

**Receipt leakage risk** → Include negative redaction assertions for prompts, credentials, headers, provider payloads, raw tool args, and secret env values when receipts are emitted.

## Validation Plan

- `openspec validate brick-05-tui-action-menu-kit --strict --json`
- Focused Rust test/example/checker for `tui-action-menu-kit`
- `git diff --check`
- Existing acceptance rail if the change touches embedded SDK, docs generation, policy, or process/job receipts
