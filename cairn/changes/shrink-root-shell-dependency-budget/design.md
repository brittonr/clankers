# Design: Shrink Root Shell Dependency Budget

## Context

The root crate is the CLI/TUI/daemon application shell, so it naturally wires many crates together. The coupling problem is not the count alone; it is root-owned reusable policy hidden inside mode, command, runtime service, prompt, and tool modules. The current inventory gives owner receipts but does not force a monotonically shrinking budget.

## Decisions

### 1. Separate legitimate app-edge wiring from temporary policy

**Choice:** Each root dependency row is classified as app-edge wiring, edge projection, adapter exception, or temporary policy.

**Rationale:** Some dependencies should remain in root forever, but reusable policy should have an owner and a convergence target. This prevents count-only churn and focuses effort where root owns behavior.

### 2. Drain root policy by behavior slice

**Choice:** Move one behavior family at a time: provider runtime services, storage/session setup, prompt/skill discovery, process/tool policy, plugin/gateway wiring, display projection, or daemon/session assembly.

**Rationale:** Root modules are broad and user-facing. Slice-level drains can preserve behavior with focused tests instead of rewriting the shell at once.

### 3. Keep root as parser, assembler, and projector

**Choice:** After a slice drains, root code may parse CLI/TUI inputs, assemble concrete services, choose adapters, and project user output, but the reusable decision logic must live in the owner crate or neutral adapter.

**Rationale:** Functional-core / imperative-shell boundaries are clearer when product-specific shell effects stay in root while reusable policy is testable outside it.

### 4. Budget evidence drives closeout

**Choice:** The root owner receipt must show a lower temporary-policy count, a lower internal-dependency budget, or narrowed exception labels with source hashes.

**Rationale:** Without budget evidence, root cleanup can become prose-only and regress silently.

## Risks / Trade-offs

- Moving behavior out of root can break CLI/TUI parity if local projection side effects are not tested.
- Some app-edge dependencies are intentionally permanent; the budget must not force fake abstractions for pure wiring.
- Generated ownership receipts need to stay synchronized with root Cargo and source changes.
