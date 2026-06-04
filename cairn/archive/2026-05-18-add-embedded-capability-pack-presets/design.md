## Context

The embedded SDK boundary intentionally keeps permissioning product-owned and explicit. The canonical spec already calls out named capability packs such as `read_only`, `networkless_coding`, `project_local_edit`, `human_approved_shell`, and `embedding_safe`, but the current adapter crate only exposes broader names (`read_only`, `tool_user`, `operator`). That leaves docs and APIs slightly misaligned and makes it harder for products to reason about safe defaults.

## Goals / Non-Goals

**Goals:**

- Provide the named product-facing capability-pack presets from the canonical spec.
- Make every preset's exact capability set deterministic and test-asserted.
- Keep safe/default packs narrow and dangerous powers opt-in by name and capability content.
- Keep the implementation inside the green `clankers-adapters` surface with no shell/runtime dependencies.

**Non-Goals:**

- Designing a full authorization engine, user approval UI, or UCAN integration for embedded products.
- Adding daemon/TUI/provider/router storage or runtime policy dependencies to the generic SDK.
- Guaranteeing these names map one-to-one to Clankers desktop permission modes.
- Changing catalog validation semantics beyond using existing `EmbeddedCapability` and approval/redaction safety rules.

## Decisions

### 1. Named constructors over a new policy trait

**Choice:** Add named constructors/helpers on `CapabilityPack` for the five product-facing presets.

**Rationale:** The existing `CapabilityPack` type is already the small data holder products can inspect. A new trait or policy engine would be premature and harder to snapshot.

**Alternative:** Add a reusable `CapabilityPolicy` trait. Rejected for this slice because the immediate need is deterministic preset evidence, not dynamic policy evaluation.

### 2. Exact capability snapshots are the guardrail

**Choice:** Tests assert each preset's full ordered `Vec<EmbeddedCapability>` output.

**Rationale:** Capability expansion is a safety-sensitive API drift. A contributor adding `Shell`, `Network`, `Mutate`, `RawLog`, or `SecretAdjacent` to a safe preset should hit a focused failing test before readiness can be claimed.

### 3. Backward-compatible aliases may remain

**Choice:** Keep existing constructors such as `tool_user()` and `operator()` as aliases if they are already public, but steer docs/tests to the product-facing names.

**Rationale:** This avoids unnecessary API churn while making the embedding contract clearer.

## Risks / Trade-offs

**[Preset names imply too much]** → Docs should describe them as capability presets, not complete security policy. Products still own enforcement at tool/provider boundaries.

**[Dangerous pack accidentally treated as default]** → Tests and docs should identify `human_approved_shell` as explicit opt-in; minimal examples should not select it implicitly.

**[Overfitting before product dogfood]** → Keep capability sets small and data-only. Defer richer policy schemas until product integrations need them.
