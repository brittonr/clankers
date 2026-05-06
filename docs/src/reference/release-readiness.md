# Release Readiness

Use this checklist when turning a Clankers change into a release candidate or when making a production-readiness claim. The pure gates stay credential-free; the live gate intentionally uses the local Lemonade/Qwen3.6 OpenAI-compatible endpoint rather than an OpenAI OAuth-backed account.

## Baseline gate

From the repository root:

```bash
./scripts/test-harness.sh full
```

This is the normal readiness harness. It runs formatting, workspace tests, clippy, repository verification rails, and Tigerstyle. Treat `target/test-harness/summary.md` or `target/test-harness/results.json` as the pass/fail source of truth; inspect per-step logs only for failed steps.

Required result before a release/readiness claim:

- `failed: 0`
- no unexpected skips in the steps that should be available on the machine
- clean `git status --short --branch` after any fixes

## Live local-model smoke

Before calling a candidate runtime-ready on a machine that can reach the local model endpoint, run:

```bash
./scripts/test-harness.sh live aspen2-qwen36
```

This runs `tests/aspen2_qwen36_integration.rs` against the aspen2 Lemonade OpenAI-compatible Qwen 3.6 endpoint. Defaults:

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

## Release-candidate checklist

1. Start from a clean branch that tracks the intended release branch.
2. Run `./scripts/test-harness.sh full` and confirm the summary reports no failures.
3. Run `./scripts/test-harness.sh live aspen2-qwen36` when the Lemonade/Qwen3.6 endpoint is reachable.
4. Confirm OAuth login commands print authorization URLs instead of opening a browser automatically.
5. Inspect `git diff --check`, commit the verified changes, push, and verify `main`/`origin/main` match.
6. Include both harness summaries in the release/readiness note, clearly separating pure readiness from optional live local-model coverage.

Do not make a general external-production claim from these gates alone. They support trusted/internal dogfooding and release-candidate hygiene; public unattended production readiness still depends on the active roadmap, security boundary review, packaging/deployment surface, and operator documentation.
