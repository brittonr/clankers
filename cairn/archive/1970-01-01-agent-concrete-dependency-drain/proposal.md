# Change: Agent Concrete Dependency Drain

## Problem

`clankers-agent` still imports concrete provider, database, and configuration crates in reusable turn, compaction, and tool-context paths. Existing ports reduced some edges, but provider request construction, DB/search access, and settings-derived policy still leak into reusable agent code.

## Goals

- Inventory concrete agent dependency edges and assign convergence owners.
- Move one provider, DB/search, or config dependency family behind a neutral agent port or app-edge adapter.
- Keep desktop behavior and agent tests stable while reducing the dependency budget.

## Non-goals

- Do not rewrite the whole agent turn loop in one slice.
- Do not remove provider integration from the application edge.
- Do not hide concrete dependencies by moving them into test-only code without an owner receipt.

## Proposed scope

Start with one concrete dependency family, preferably compaction/model request construction or DB/search access in tool execution, and replace direct concrete imports with neutral ports plus focused adapter tests.

## Verification

Focused validation should include agent turn/compaction tests, dependency inventory rails, `cargo check -p clankers-agent --tests`, Cairn gates, and `git diff --check`.
