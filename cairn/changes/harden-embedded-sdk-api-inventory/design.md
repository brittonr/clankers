# Design: Harden Embedded SDK API Inventory

## Context

The current inventory checker reads Rust source line-by-line and recognizes only simple `pub struct`, `pub enum`, `pub fn`, `pub const`, `pub mod`, `pub trait`, and `pub type` declarations. It intentionally ignores `pub use`, methods inside `impl`, fields inside public structs, and feature conditions. Recent SDK cleanup relies increasingly on inventory labels, so blind spots matter.

## Decisions

### 1. Typed inventory over source lines

Use a typed parser or generated metadata to collect public API items. The rail should record enough owner context to distinguish top-level items, methods, fields, enum variants when relevant, and reexports.

### 2. Stable-contract hash remains deterministic

Inventory rows should be sorted deterministically and stable-contract hashing should include only supported/optional/compatibility items so unsupported/internal churn does not force migration notes.

### 3. Diagnostics name owners and replacement paths

When a public SDK item is missing or misclassified, diagnostics should name the source file, item path, stability expectation, and whether the item should be classified, hidden, or moved to an app-edge compatibility boundary.

## Risks / Trade-offs

- Full rustdoc JSON may be brittle in Nix/toolchain contexts; a `syn` parser over source may be more stable for repository rails.
- Adding methods/fields will expand inventory counts and require a deliberate policy refresh.
- Reexport handling can be noisy; start with root reexports in green SDK crates and expand from there.
