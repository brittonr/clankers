# Design: Drain Root Shell Policy

## Boundary

Root modules may own imperative shell assembly only:

```text
CLI/TUI/daemon input -> root parser/wiring -> owned service/adapter -> neutral receipt/event -> edge projection
```

Root modules must not become the canonical owner for reusable business rules, storage schemas, provider-native body shaping, process backend policy, prompt/skill lookup semantics, plugin runtime lifecycle, session format behavior, or display/protocol DTO construction.

## Ownership map

Each root dependency edge should be classified as one of:

- `shell-wiring`: root constructs and injects a concrete service, but policy is tested in the target owner.
- `edge-projection`: root converts neutral DTOs into CLI/TUI/daemon output only.
- `temporary-policy`: root still owns reusable behavior and needs a named drain target plus convergence condition.
- `adapter-exception`: root owns a deliberately product-specific adapter with focused tests and no green SDK claim.

The inventory should name the module path, target owner, DTOs crossing the seam, and the validation rail that prevents backsliding.

## First implementation slice

Choose one root policy cluster that is both reusable and currently root-owned. Prefer a cluster with deterministic fixtures and low user-visible risk, such as runtime service assembly policy, session setup policy, or a root tool projection that can call an existing typed service.

The slice should move policy down or outward to the named owner, update root code to delegate, and refresh the lego dependency ownership baseline with a smaller convergence condition.

## Verification

Validation must show the root module is thinner after the slice: focused tests at the new owner, no new forbidden constructor/imports in root policy paths, and updated owner receipts in the architecture rail.
