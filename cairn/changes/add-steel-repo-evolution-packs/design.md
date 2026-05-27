# Design: repo-local Steel evolution packs

## Overview

A Steel evolution pack is a repo-local, runtime-loaded orchestration bundle. The repository owns policy and workflow shape; Rust owns host calls, validation, execution, and receipts.

Default root:

```text
.clankers/steel/
  evolution-profile.ncl
  scripts/
    plan-evolution.scm
    select-gates.scm
  receipts/
```

`evolution-profile.ncl` is the source of truth. The runtime may consume an exported JSON form, but Nickel validation and generated fixtures must prove the exported shape before Rust accepts it.

## Host ABI

Rust exposes a small versioned ABI, for example:

- `repo.read_context` — safe metadata and bounded text snippets only.
- `repo.propose_patch` — typed patch proposal, no apply.
- `repo.run_gate` — request a policy-allowed local gate through Rust.
- `repo.record_receipt` — write redacted evidence through Rust.
- `repo.ask_human` — emit a checkpoint request.

Unknown host calls fail closed. ABI additions require Rust code and tests, not Steel pack edits.

## Pack activation

Activation is default-deny:

- No `.clankers/steel/evolution-profile.ncl` means no repo-local evolution pack.
- A present pack must validate its Nickel contract, exported schema, script paths, BLAKE3 hashes, budgets, allowed host calls, gate list, receipt root, and fallback policy.
- Pack paths must stay under `.clankers/steel/` unless a future explicit policy permits additional roots.
- Reload is hash/version driven. A changed pack becomes active only after a successful validation receipt.

## Typed evolution plans

Steel emits a typed `clankers.steel.evolution-plan.v1` plan. Rust rejects free-form text, malformed plans, unknown actions, and over-budget requests. Plans may ask Rust to inspect repo context, propose patches, run gates, or ask for human input; plans cannot perform those effects directly.

## Receipts

Every activation and accepted plan records safe evidence:

- profile hash
- script hashes
- ABI version
- allowed host call set
- denied host call set when applicable
- gate selection
- plan hash
- fallback/block outcome

Receipts must not include raw prompts, credentials, compact UCAN tokens, provider payloads, secrets, or unbounded script source.

## Rollout

Implementation should start as an explicit setting or CLI command before becoming automatic per-turn behavior. The pack is useful even in dry-run mode because it can emit repo-specific evolution plans without mutating the repository.
