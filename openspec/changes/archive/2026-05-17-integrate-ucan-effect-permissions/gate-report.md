# OpenSpec Gate Report

## Change

`integrate-ucan-effect-permissions`

## Initial Gate Findings

Three strict review passes were run for proposal, design, and tasks. All initially returned `FAIL` with consistent findings:

- Modified delta specs dropped existing scenario IDs/scenarios for `effect-ability-runtime`, `content-addressed-agent-artifacts`, and `typed-durable-session-ledger`.
- Protected effect coverage was inconsistent: provider, delivery, plugin, and MCP appeared in some requirements but not the stable ability vocabulary.
- Human-confirmation ordering was identified as a risk/non-goal but lacked requirement, scenario, and task coverage.
- Caveat policy scenarios covered only path/command and unknown caveats while the proposal/design claimed network, provider/model, artifact, redaction, replay/freshness, time, and max-byte caveats.
- The task marker `ucan-effect-permissions.remote-proof-sync` had no matching requirement/scenario.
- Several implementation tasks were broad bundles without concrete evidence receipts.

## Fixes Applied

- Restored existing modified-spec scenario IDs and scenarios:
  - `effect-ability-runtime.handlers.absent-fail-closed`
  - `effect-ability-runtime.handlers.simulate`
  - `effect-ability-runtime.handlers.replay`
  - `effect-ability-runtime.remote-deps.missing-safe`
  - `effect-ability-runtime.remote-deps.secret-denied`
  - `content-addressed-agent-artifacts.receipts.replay`
  - `content-addressed-agent-artifacts.receipts.redaction`
  - `typed-durable-session-ledger.records.execution`
  - `typed-durable-session-ledger.records.redaction`
- Added confirmation-order requirements/scenarios:
  - `ucan-effect-permissions.handler-admission.confirmation-order`
  - `effect-ability-runtime.handlers.confirmation-order`
- Aligned the stable ability vocabulary across proposal/design/specs to include file, shell, network, secret, browser, scheduler, remote, provider, delivery, artifact, plugin, and MCP classes.
- Added deterministic caveat scenarios for path/command, network/provider, artifact/redaction, freshness, and unknown-caveat denial.
- Added `ucan-effect-permissions.remote-proof-sync` with safe-reference and missing-authority scenarios.
- Split tasks into smaller ordered slices with concrete `[covers=...]` and `[evidence=...]` markers.

## Validation

- Proposal gate: PASS on rerun; no actionable findings.
- Design gate: PASS on rerun; no actionable findings.
- Tasks gate: PASS on rerun; no actionable findings.
- `openspec validate integrate-ucan-effect-permissions --strict --json`: PASS (`valid: true`, 0 issues).
- `git diff --check -- openspec/changes/integrate-ucan-effect-permissions`: PASS.
