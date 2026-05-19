# Plugin/tool runtime dispatch separation Design

## Context

The embedded SDK now has green crates, reusable adapter bricks, executable recipes, lego policy evidence, and an archived `embedded-composition-kits` canonical spec. The next work should improve product composability without importing shell/runtime ownership into generic SDK crates.

## Goals / Non-Goals

**Goals:** create one narrow, verifiable lego brick or product evidence rail; keep behavior fixture-backed and content-addressed where claims need drift detection; update docs and acceptance rails with the implementation.

**Non-Goals:** do not move daemon sockets, TUI/rendering, provider discovery, OAuth stores, Clankers DB/session ownership, Matrix, iroh/P2P, plugin supervision, built-in tool bundles, or Nickel runtime evaluation into green SDK crates.

## Decisions

### 1. Keep generic bricks shell-free

**Choice:** implement reusable behavior in green SDK crates or examples using explicit host-owned inputs.

**Rationale:** lego-like composition depends on replaceable adapters and typed data, not hidden singletons or app-shell ownership.

**Alternative:** importing existing Clankers runtime crates would be faster but would collapse the product embedding boundary.

### 2. Verify with receipts and focused examples

**Choice:** pair each slice with focused tests/examples plus acceptance-rail or receipt evidence.

**Rationale:** downstream embedders need inspectable proof that the advertised brick is stable and dependency-clean.

## Risks / Trade-offs

**Over-generalization** → Only promote APIs after product evidence or repeated example convergence.

**Checker-only confidence** → Keep executable examples/tests for runtime behavior and use BLAKE3 receipts for drift detection, not authorization.
