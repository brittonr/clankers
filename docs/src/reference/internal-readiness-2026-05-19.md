# Internal Readiness Checkpoint 2026-05-19

`internal-readiness-2026-05-19` is an internal/trusted dogfood checkpoint for Clankers. It is not a public unattended-production readiness claim.

## Identity

- Tag: `internal-readiness-2026-05-19`
- Commit: `7e7737dde3530a0f3019930d47c702a3b610d6ef`
- Short commit: `7e7737dd Refresh engine host matrix receipt guard`
- Scope: internal/trusted dogfood readiness for embedded SDK and adjacent-product integration work

## What this checkpoint proves

- The embedded SDK release receipt was regenerated after `main` was pushed and aligned with `origin/main`.
- The receipt records schema `clankers.embedded_sdk.release_receipt.v1`.
- The receipt records git status `## main...origin/main`.
- The receipt hashes 49 embedded SDK evidence artifacts under docs, examples, OpenSpec specs, and release/check scripts.
- The embedded SDK boundary is explicit:
  - green generic SDK crates include `clanker-message`, `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, `clankers-adapters`, and the optional prompt lifecycle reducer in `clankers-core`;
  - red exclusions keep daemon protocol clients, TUI, provider discovery/router/OAuth stores, session database ownership, plugin supervision, Matrix, iroh/P2P, built-in tool bundles, live credentials, network access, daemon startup, and shell-global service lookup outside the generic SDK boundary;
  - yellow app-edge surfaces remain product-owned integration layers.
- The maintained receipt rail lists these verification commands:
  - `scripts/check-embedded-agent-sdk.rs`
  - `nix build .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).embedded-sdk-release-receipt`
  - `cargo check --workspace --all-targets`
  - `openspec validate embedded-composition-kits --strict --json`
  - `cargo fmt --check`
  - `git diff --check`
- The final readiness fix refreshed `scripts/check-engine-host-feature-matrix.rs` to track the current engine-host test name before receipt regeneration.
- The latest full harness evidence before the tag was green: run `20260519T193436Z-1932474`, `6/6` passed, `0` failed.
- Adjacent Remora/changebot dogfood evidence validated Clankers as an external-product reasoning backend with primary `openai-codex/gpt-5.3-codex` accepted at attempt index `0` and no fallback acceptance in durable state `/home/brittonr/remora-operator-state/tile-clankers-gpt53codex-20260519T213556Z`.
- Remora now rejects unsupported or unproven Codex primary drift in the dogfood fixture so fallback success cannot mask a broken Clankers primary model.

## What this checkpoint does not prove

- It does not claim unattended public production readiness.
- It does not prove every host-dependent VM, flake, live-provider, network, or deployment surface is green on every machine.
- It does not remove the need for operator review before public releases, deployment packaging, credential/security boundary review, or broader live dogfood coverage.
- It does not make `openai-codex/gpt-5.5` a Remora dogfood default; direct transport smoke passed, but structured Remora review failed with `invalid_response_json`, so the proven primary remains `openai-codex/gpt-5.3-codex`.

## Evidence locations

- Embedded SDK receipt: `target/embedded-sdk-release/receipt.json`
- Full harness summary: `target/test-harness/summary.md`
- Full harness results: `target/test-harness/results.json`
- Full harness logs: `target/test-harness/runs/20260519T193436Z-1932474/logs/`
- Durable Remora/Clankers dogfood state: `/home/brittonr/remora-operator-state/tile-clankers-gpt53codex-20260519T213556Z`

## Recommended next evidence

1. Run another report-only external-product dogfood target against this tag and preserve durable operator state.
2. Promote the embedded SDK receipt rail into a routine CI/check entry so receipt freshness does not depend on manual release preparation.
3. Continue product hardening with a scoped OpenSpec when changing process/job profile behavior or public agent surfaces.
