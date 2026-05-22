# Proposal: Steel Turn Planning Runtime Smoke

## Summary

Add a deterministic runtime smoke rail proving reviewed Steel Scheme `steel.host.plan_turn` activation flows through a real Clankers session boundary, emits redacted planning receipt evidence, and fails closed when reviewed config authority or hashes are wrong.

## Motivation

The config activation slice made Steel turn planning selectable from reviewed settings/profile/script files. The next risk is integration drift: real daemon/session construction, prompt handling, and attach-visible event replay could bypass that activation helper or lose receipt evidence. A focused smoke rail closes that gap without granting Steel ambient authority.

## Scope

- Add deterministic session/controller-level smoke coverage for settings-driven Steel turn planning activation.
- Prove default-disabled and fail-closed invalid-profile/hash/authority behavior at the runtime session seam.
- Add a repo-local checker that records redacted smoke evidence under `target/`.
- Document the smoke boundary and its non-claims.

## Non-goals

- No live provider/network test.
- No real user credentials or UCAN tokens.
- No filesystem, shell, git, network, credential, provider, daemon mutation, or TUI authority for Steel.
- No claim that Steel is a sandbox.
