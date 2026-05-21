## Why

Clankers readiness work has treated the aspen2 Qwen rail as an optional live smoke while recent external dogfood evidence still named Codex as the accepted primary and qwen/aspen2 as fallback. For the current testing/readiness class of work, operators need the repo-owned guidance and checks to make qwen on aspen2 the preferred live model path instead of an oral convention.

## What Changes

- Document qwen/aspen2 as the primary live testing/readiness model for Clankers dogfood and release-readiness slices.
- Keep credential-free gates pure and keep OpenAI OAuth/Codex separate from this live testing path.
- Add a small regression assertion so release-readiness docs continue to name qwen/aspen2 as the primary live testing model and the existing `aspen2-qwen36` harness/nextest seam stays discoverable.
- Preserve a fresh report-only qwen/aspen2 live harness receipt when the endpoint is reachable, or an explicit unavailable receipt if it self-skips/fails prerequisite probing.

## Impact

- **Files**: `cairn/changes/qwen-aspen2-readiness-primary/**`, `docs/src/reference/release-readiness.md`, `README.md`, `tests/release_readiness_docs.rs`, and a release evidence note if a fresh receipt is preserved.
- **Testing**: focused release-readiness docs test, Cairn validate/gates, mdBook build, and `./scripts/test-harness.sh live aspen2-qwen36` with qwen on aspen2.
