# Design: Resolve Experimental SDK Ports

## Context

The inventory labels `CostAccountingPort`, `PromptHistoryPort`, `PersistencePort`, `HookPort`, many tool-host service DTOs, and neutral invocation context APIs as experimental. Some are referenced in docs as adapter contracts, but not all are exercised by green examples or runner paths.

## Decisions

### 1. Treat experimental as a budget

Create a small policy inventory that counts experimental rows by crate and owner, with an expected convergence decision for each group.

### 2. Promote only dogfooded seams

A port can become supported only when at least one deterministic fixture or example uses it through the public API and negative behavior is covered.

### 3. Hide unused ports

Ports that are not wired and not required by a near-term product recipe should become private or remain behind an explicit experimental module with no stable compatibility promise.

## Risks / Trade-offs

- Making items private can break internal tests that relied on public visibility; move those tests next to owners or use supported adapters.
- Stabilizing too much can freeze poor names; require docs and fixtures first.
- Tool-host service APIs are broad; resolve in batches rather than one giant API redesign.
