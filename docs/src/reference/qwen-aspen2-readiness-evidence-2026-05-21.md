# Qwen/aspen2 Readiness Evidence 2026-05-21

This note records the first Clankers readiness slice after switching this class of testing/dogfood/readiness work to **qwen on aspen2** as the primary live testing model path.

## Scope

- Repository: `/home/brittonr/git/clankers`
- Change package: `qwen-aspen2-readiness-primary`
- Live model path: qwen on aspen2 via `./scripts/test-harness.sh live aspen2-qwen36`
- Endpoint/model defaults exercised by the integration test:
  - `ASPEN2_QWEN36_BASE_URL=http://aspen2:13305/v1`
  - `ASPEN2_QWEN36_MODEL=user.Qwen3.6-35B-A3B`
- OpenAI OAuth/Codex substitution: not used for this live testing receipt

## Harness receipt

- Run id: `qwen-aspen2-readiness-primary-20260521T150524Z`
- Run dir: `target/test-harness/runs/qwen-aspen2-readiness-primary-20260521T150524Z`
- Started: `2026-05-21T15:09:16Z`
- Finished: `2026-05-21T15:10:13Z`
- Passed: `1`
- Failed: `0`
- Skipped: `0`

Executed step:

```text
env CLANKERS_RUN_LIVE_READINESS=1 CLANKERS_LIVE_READINESS_SELECTOR=aspen2-qwen36 cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast -E test(readiness_live_local_model_aspen2_qwen36_nextest_opt_in)
```

The nextest receipt reported:

```text
PASS [56.024s] clankers::readiness_opt_in readiness_live_local_model_aspen2_qwen36_nextest_opt_in
Summary [56.025s] 1 test run: 1 passed, 2 skipped
```

## Interpretation

This proves the repo-owned live readiness seam accepted qwen/aspen2 as the primary live testing path for this slice. It does not claim public unattended production readiness, VM readiness, flake readiness, or OpenAI/Codex stability. Those remain separate opt-in rails and must be recorded independently when used.
