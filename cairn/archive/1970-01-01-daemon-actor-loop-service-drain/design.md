# Design: Daemon Actor Loop Service Drain

## Context

Previous work moved some session assembly into builders, but the actor loop still owns several policies that can be socketless services. This slice continues the actor-loop drain.

## Decisions

### 1. Actor loop receives services

Assembly should prepare service handles and policies before `run_agent_actor` begins polling.

### 2. Drain one tick responsibility at a time

Extract a small service whose behavior can be tested without a registry or Unix socket.

### 3. Preserve event ordering

Actor-loop extraction must maintain ordering for prompt, cancel, plugin, and controller events.

## Risks / Trade-offs

- Event ordering regressions can be subtle; keep focused runtime seam tests.
- Plugin state is concurrent; avoid locks held across awaits.
- Socketless tests can miss registry behavior; keep one integration-style daemon test when needed.
