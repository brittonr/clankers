# Release Readiness

Use this checklist when turning a Clankers change into a release candidate or when making a production-readiness claim. The pure gates stay credential-free; for Clankers testing, dogfood, and release-readiness slices that require a live model, qwen on aspen2 is the primary live testing model path. That live gate intentionally uses the local Lemonade/Qwen3.6 OpenAI-compatible endpoint rather than an OpenAI OAuth-backed account.

## Baseline gate

From the repository root:

```bash
./scripts/test-harness.sh full
```

This is the normal readiness harness. It runs formatting, workspace tests, clippy, repository verification rails, and Tigerstyle. Treat `target/test-harness/summary.md` or `target/test-harness/results.json` as the pass/fail source of truth; inspect per-step logs only for failed steps. After one or more readiness profiles have produced local receipts, run `./scripts/test-harness.sh evidence-index` to compose the current Git/lifecycle state with the latest valid local receipts under `target/release-evidence/current-head/`. The index does not run missing profiles and must not be treated as evidence for modes it reports as missing.

The assertion layer for release-readiness gaps is Rust/nextest-owned. The credential-free E2E tier is:

```bash
cargo nextest run -p clankers --test readiness_e2e --no-fail-fast
```

The legacy `tests/e2e/run-tests.sh` script and `./scripts/test-harness.sh e2e ...` are compatibility wrappers around that nextest test binary; they do not own the readiness assertions.

Required result before a release/readiness claim:

- `failed: 0`
- no unexpected skips in the steps that should be available on the machine
- clean `git status --short --branch` after any fixes

## Live local-model smoke

Before calling a candidate runtime-ready on a machine that can reach the local model endpoint, run the opt-in nextest adapter directly or through the harness:

```bash
CLANKERS_RUN_LIVE_READINESS=1 \
  cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast \
  -E 'test(readiness_live_local_model_aspen2_qwen36_nextest_opt_in)'

./scripts/test-harness.sh live aspen2-qwen36
```

This runs `tests/aspen2_qwen36_integration.rs` against the aspen2 Lemonade OpenAI-compatible Qwen 3.6 endpoint. Treat qwen on aspen2 as the primary live testing model for this workstream; do not substitute Codex/OpenAI OAuth smoke checks unless the task explicitly asks for that provider. Defaults:

- `ASPEN2_QWEN36_BASE_URL=http://aspen2:13305/v1`
- `ASPEN2_QWEN36_MODEL=user.Qwen3.6-35B-A3B`

Use this as the preferred live runtime smoke because it exercises streaming/reasoning-or-text behavior without launching OpenAI OAuth or browser login flows. The test self-skips when the endpoint/model is unavailable; record that as "live local-model unavailable" rather than substituting an OAuth-backed OpenAI login unless that is explicitly requested.

For the Nix check form of the same live seam:

```bash
CLANKERS_ENABLE_LIVE_CHECKS=1 \
  nix build --impure --no-link \
  .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).live-aspen2-qwen36 \
  --option sandbox false -L
```

## Opt-in VM and flake readiness

VM and flake/CI readiness are also nextest-owned and gated by explicit environment variables so default workspace tests remain credential-free and bounded:

```bash
CLANKERS_RUN_VM_READINESS=1 \
  cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast \
  -E 'test(readiness_vm_required_nixos_checks_nextest_opt_in)'

CLANKERS_RUN_FLAKE_READINESS=1 \
  cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast \
  -E 'test(readiness_flake_ci_nextest_opt_in)'
```

`CLANKERS_VM_READINESS_SELECTOR` may be `all`, `core`, `module`, `smoke`, or an explicit check name such as `vm-plugin-runtime`; the harness sets it from `./scripts/test-harness.sh vm <selector>`. Do not claim a VM or flake pass unless the corresponding opt-in env var was set and the nextest adapter actually ran.

## Release-candidate checklist

1. Start from a clean branch that tracks the intended release branch.
2. Run `./scripts/test-harness.sh full` and confirm the summary reports no failures.
3. Run `cargo nextest run -p clankers --test readiness_e2e --no-fail-fast` if the baseline summary was produced by older tooling or if you need a focused credential-free E2E receipt.
4. Run `nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).embedded-sdk-release-receipt` to verify the embedded SDK receipt rail remains wired into the routine Nix check surface.
5. Run `./scripts/test-harness.sh live aspen2-qwen36` when the Lemonade/Qwen3.6 endpoint is reachable.
6. Run `./scripts/test-harness.sh vm all` and `./scripts/test-harness.sh ci` on machines authorized for NixOS VM and flake-heavy checks.
7. Run `./scripts/test-harness.sh evidence-index` after the selected profiles complete, then inspect `target/release-evidence/current-head/index.md` for selected receipts, missing modes, dirty status, and non-claims.
8. Confirm OAuth login commands print authorization URLs instead of opening a browser automatically.
9. Inspect `git diff --check`, commit the verified changes, push, and verify `main`/`origin/main` match.
10. Include the evidence index plus the full harness summary and any opt-in nextest live/VM/flake summaries in the release/readiness note, clearly separating pure readiness from optional host-dependent coverage.

Do not make a general external-production claim from these gates alone. They support trusted/internal dogfooding and release-candidate hygiene; public unattended production readiness still depends on the active roadmap, security boundary review, packaging/deployment surface, and operator documentation.
