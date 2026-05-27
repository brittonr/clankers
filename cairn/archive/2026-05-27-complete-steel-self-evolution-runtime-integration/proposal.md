# Complete Steel Self-Evolution Runtime Integration

## Summary

Repair the overclaimed Steel self-evolution archive by adding the missing runtime seams: repo-local evolution packs load during real turn planning, allowed Steel host calls require higher-order contracts, and orchestration pack mutation performs real isolated staging before promotion.

## Problem

The archived Steel self-evolution work implemented validation DTOs and checkers but did not prove the claimed runtime integration. Review found that `load_repo_evolution_pack(...)` had no turn/startup call site, Nickel policy checks were shallow, orchestration mutation did not apply an isolated candidate, and the self-mutation spec lost rollback/fixture requirements.

## Goals

- Load `.clankers/steel/evolution-profile.{ncl,json}` from the actual agent turn paths before turn planning.
- Keep missing repo-local packs silent/default-deny and invalid packs fail-closed.
- Require higher-order host contracts around every allowed repo-evolution host call.
- Add repo-local `.clankers/steel/` pack data with Nickel-authored contract source and matching JSON export.
- Stage orchestration mutation payloads into an isolated directory before promotion receipts.
- Restore self-mutation rollback, safe-receipt, raw-write-denial, and positive/negative fixture requirements.
- Record focused verification evidence without claiming archive completion early.

## Non-Goals

- Steel still cannot write directly to the live repo, shell, git, network, providers, credentials, daemon, TUI, or capability grants.
- This change does not add automatic commits or pushes from Steel output.
- This change does not make Steel authority-kernel widening self-approvable.
