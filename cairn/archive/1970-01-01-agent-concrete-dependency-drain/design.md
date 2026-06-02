# Design: Agent Concrete Dependency Drain

## Context

The prior drain removed direct router/TUI normal dependencies from `clankers-agent`, but `clankers_provider`, `clankers_db`, and `clankers_config` are still visible in reusable modules. The target is a shrinking, measured budget rather than an all-at-once purge.

## Decisions

### 1. Move one dependency family per slice

Choose provider request construction, storage/search access, or settings-derived policy and drain it completely before starting another family.

### 2. Ports must be semantically neutral

A port should expose agent needs, not provider/router/database implementation vocabulary.

### 3. Keep builders as app-edge composition

Concrete providers, DB handles, and settings may remain in builders/adapters when those owners are explicit and tested.

## Risks / Trade-offs

- Too broad a dependency sweep can destabilize turn behavior; slice by family.
- A neutral port can become a concrete wrapper in disguise; review DTO names and tests.
- Tests may still need provider fixtures; keep test-only imports out of production dependency budgets.
