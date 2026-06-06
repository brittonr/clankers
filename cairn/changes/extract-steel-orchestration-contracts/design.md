# Design: Extract Steel Orchestration Contracts

## Boundary

The neutral owner contains serializable plan requests, plan decisions, host-call request/response DTOs, script metadata, and repo-evolution pack manifests. It must not execute Steel, read scripts from disk, resolve Nickel profiles, mutate repositories, access clocks, or call host services.

## Runtime adapter

`clankers-runtime` remains responsible for executable behavior: loading bundled/default scripts, dispatching host calls, validating hash-bound repo packs against files, and applying mutation policies. Adapters convert between the green DTOs and runtime execution services.

## Rails

Runtime facade inventory should show fewer yellow Steel public rows or explicitly classify them as adapter-only. Repo-evolution pack checks should prove source hashes remain synchronized after any split.
