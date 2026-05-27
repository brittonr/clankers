# Steel Repo Evolution Packs

Repo-local Steel evolution packs let a repository customize orchestration without recompiling Clankers. The pack is runtime-loaded data; Rust remains the authority boundary.

## Layout

```text
.clankers/steel/
  evolution-profile.ncl   # Nickel source contract
  evolution-profile.json  # exported typed data consumed by Rust
  scripts/
    plan-evolution.scm
    select-gates.scm
```

If `.clankers/steel/evolution-profile.ncl` is absent, repo-local Steel evolution is inactive. Clankers continues with the bundled/default orchestration path and emits no repo-local Steel authorship claim.

## Nickel profile contract

The Nickel profile declares the pack schema, ABI version, script bindings, BLAKE3 hashes, budgets, allowed host calls, higher-order host contracts, gate names, receipt root, and fallback mode. Rust consumes exported typed data and validates every field before activation. Invalid exports, invalid Nickel contract markers, path escapes, missing scripts, hash mismatches, unknown host calls, missing higher-order contracts, escaped receipt roots, and over-budget scripts fail closed before Steel code runs.

Each allowed host call must have a `host_contracts` entry with `mode = "higher_order"`, preconditions, and postconditions. The contract wraps Steel's proposed call and keeps Rust-owned validation, staging, gates, receipt emission, promotion, and rollback between Steel output and any effect.

## Runtime loading

`Agent::run_turn_loop` and the orchestrated turn path call `load_repo_evolution_pack(...)` from the current repository before planning a turn. Missing packs are silent/default-deny. Present packs emit a safe system receipt status after Rust validation; activation does not execute repo-local Steel source until hashes, budgets, ABI, and higher-order host contracts pass.

The repository now carries a default pack under `.clankers/steel/` as data, not Rust code. Changing that pack requires updating `evolution-profile.ncl`, exporting matching `evolution-profile.json`, and keeping script BLAKE3 hashes in sync.

## Stable Rust host ABI

The first repo evolution ABI is intentionally narrow:

- `repo.read_context`
- `repo.propose_patch`
- `repo.run_gate`
- `repo.record_receipt`
- `repo.ask_human`

Unknown host calls fail closed. Adding host calls, widening budgets, or granting new authority requires Rust code and review; a repo-local Steel pack cannot mint those powers by editing itself.

## Typed plans

Steel emits `clankers.steel.evolution-plan.v1`. Rust parses the typed plan, rejects free-form output, checks selected gates against the pack policy, and authorizes each requested host action separately. The plan may request bounded reads, patch proposals, gate runs, receipt recording, or human checkpoints. It cannot apply patches, run shell, access credentials, call providers, mutate sessions, or push code directly.

## Receipts

Activation and plan receipts include safe metadata: profile hash, script hashes, ABI version, allowed host calls, selected gates, plan hash, denied host calls, fallback/block status, and receipt hash. They omit raw prompts, credentials, compact UCAN tokens, provider payloads, secrets, raw script source, and uncontrolled absolute paths.

## Verification

Run:

```text
./scripts/check-steel-repo-evolution-packs.rs
```

The checker exercises absent, valid, repo-local runtime load, malformed, hash-mismatched, path-escaped, unknown-host-call, over-budget, valid-plan, malformed-plan, and higher-order host-contract fixtures. It writes `target/steel-repo-evolution-packs/receipt.json` with hashes for the Rust validator, checker, docs, Nickel profile, JSON export, and Steel script.
