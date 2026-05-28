# Design: Thin Agent Runtime Shell

## Summary

`clankers-agent` should become an adapter around runtime services and engine-host adapters. Concrete Clankers desktop behavior belongs in root or desktop adapter crates; the agent should not be the only place where provider, tool, session, hook, prompt, and skill policies can run.

## Decisions

### Decision: agent service bundle replaces scattered concrete fields

Introduce a narrow service bundle for provider/model execution, tool registry, prompt context, storage, hooks, skills, cost, and cancellation. The existing `Agent::new` can build this bundle for compatibility, but turn execution should read from interfaces.

### Decision: concrete dependencies need owner receipts

During migration the agent may retain concrete dependencies as adapters, but an architecture rail should report each concrete edge, owner, and target removal condition. New concrete agent dependencies should fail unless they are explicitly adapter-owned.

### Decision: shell behavior remains unchanged

Standalone, daemon/controller, and attach behavior should be preserved through parity tests while internals move behind ports.

## Verification Plan

- Add fake service fixtures that construct an agent turn without `clankers-provider`, `clankers-db`, prompt bundles, or TUI DTOs.
- Extend the lego dependency ownership rail to track the concrete dependency budget and owner reasons.
- Keep existing turn fixture parity green through the migration.
