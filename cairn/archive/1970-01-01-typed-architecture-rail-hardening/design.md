# Design: Typed Architecture Rail Hardening

## Context

Architecture rails enabled the coupling drain, but the rail implementation now has its own coupling: it is too attached to exact source spelling. This change hardens one cluster at a time.

## Decisions

### 1. Preserve diagnostics while changing mechanism

Typed rails should produce at least as useful owner diagnostics as string anchors.

### 2. Use AST for ownership, behavior for parity

Constructor ownership belongs in AST inventories; user-visible parity should be behavior tested when practical.

### 3. Document remaining anchors

If an exact source string is still the least risky check, document why and name its owner.

## Risks / Trade-offs

- AST inventories can overfit too; keep checks focused on ownership semantics.
- Cargo-script formatting is noisy; edit exact hunks only.
- Generated manifests must be kept in sync with baseline schema.
