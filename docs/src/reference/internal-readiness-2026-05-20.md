# Internal Readiness Checkpoint 2026-05-20

`internal-readiness-2026-05-20` is an internal/trusted dogfood checkpoint for Clankers after the process/job profile hardening and OpenSpec review-gate omission-prevention slices. It is not a public unattended-production readiness claim.

## Identity

- Tag: `internal-readiness-2026-05-20`
- Commit: the clean `main` commit carrying this checkpoint note
- Scope: internal/trusted dogfood readiness for embedded SDK, durable process/job profile handling, and repo-owned OpenSpec review-gate rails

## What this checkpoint proves

- The embedded SDK acceptance rail passed from a clean, pushed `main` checkout:
  - `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs`
- The routine Nix receipt check passed:
  - `nix build --no-link .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).embedded-sdk-release-receipt -L`
- The embedded SDK release receipt was regenerated after the branch was clean/aligned with `origin/main`.
- The receipt records schema `clankers.embedded_sdk.release_receipt.v1`.
- The receipt records git status `## main...origin/main`.
- The receipt hashes 50 embedded SDK evidence artifacts under docs, examples, Cairn specs, policy, and release/check scripts.
- The embedded SDK boundary remains explicit:
  - green generic SDK crates include `clanker-message`, `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, `clankers-adapters`, and the optional prompt lifecycle reducer in `clankers-core`;
  - red exclusions keep daemon protocol clients, TUI, provider discovery/router/OAuth stores, session database ownership, plugin supervision, Matrix, iroh/P2P, built-in tool bundles, live credentials, network access, daemon startup, and shell-global service lookup outside the generic SDK boundary;
  - yellow app-edge surfaces remain product-owned integration layers.
- The process/job profile hardening change is archived and validated in the canonical `durable-process-jobs` spec.
- The OpenSpec review-gate omission-prevention change is archived and validated in the canonical `openspec-review-gates` spec.
- The full Clankers readiness harness passed against this checkpoint:
  - run `20260520T161137Z-1226432`;
  - `6` steps passed, `0` failed, `0` skipped;
  - summary: `target/test-harness/runs/20260520T161137Z-1226432/summary.md`;
  - results: `target/test-harness/runs/20260520T161137Z-1226432/results.json`.

## What this checkpoint does not prove

- It does not claim unattended public production readiness.
- It does not prove every host-dependent VM, flake, live-provider, network, or deployment surface is green on every machine.
- It does not change Remora dogfood primary selection; the proven Remora primary remains `openai-codex/gpt-5.3-codex` with required fallback `qwen36-aspen2`.

## Evidence locations

- Embedded SDK receipt: `target/embedded-sdk-release/receipt.json`
- Embedded SDK acceptance rail: `scripts/check-embedded-agent-sdk.rs`
- Routine Nix receipt check: `checks.<system>.embedded-sdk-release-receipt`
- Canonical embedded SDK spec: `cairn/specs/embedded-composition-kits/spec.md`
- Canonical process/job spec: `cairn/specs/durable-process-jobs/spec.md`
- Canonical review-gates spec: `cairn/specs/openspec-review-gates/spec.md`
- Full harness summary: `target/test-harness/summary.md`
- Full harness results: `target/test-harness/results.json`

## Recommended next evidence

1. Run another report-only external-product dogfood target against this tag and preserve durable operator state.
2. Address legacy repo-wide OpenSpec validation debt so `openspec validate --all --strict` can become a routine green gate.
