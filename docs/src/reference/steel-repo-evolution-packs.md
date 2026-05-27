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

The Nickel profile declares the pack schema, ABI version, script bindings, BLAKE3 hashes, budgets, allowed host calls, gate names, receipt root, and fallback mode. Rust consumes exported typed data and validates every field before activation. Invalid exports, path escapes, missing scripts, hash mismatches, unknown host calls, escaped receipt roots, and over-budget scripts fail closed before Steel code runs.

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

The checker exercises absent, valid, malformed, hash-mismatched, path-escaped, unknown-host-call, over-budget, valid-plan, and malformed-plan fixtures and writes `target/steel-repo-evolution-packs/receipt.json`.
