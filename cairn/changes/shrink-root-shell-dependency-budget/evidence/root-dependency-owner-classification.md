Artifact-Type: validation-log
Task-ID: R1,I2
Covers: r[remaining-coupling-drain.root-shell-dependency-budget.inventory], r[remaining-coupling-drain.root-shell-dependency-budget.budget-evidence]
Status: pass

## Scope

Classified every current root `clankers` internal dependency row in the lego architecture ownership rail into one of the active drain-plan classes:

- `app-edge wiring`: root crate is selecting a user-facing product mode, command, optional integration, or desktop service assembly boundary.
- `edge projection`: root crate is translating to/from daemon, display, semantic message, or protocol-facing DTOs.
- `adapter exception`: root crate holds an explicitly named adapter around a concrete subsystem while reusable policy remains in the target crate.
- `temporary policy`: root crate still owns too much reusable behavior and the row now names a convergence target for a later drain slice.

The classification is encoded in `scripts/check-lego-architecture-boundaries.rs::root_dependency_owner(...)` and mirrored in `policy/lego-architecture/dependency-ownership-baseline.json` so the typed ownership rail will reject unclassified root dependencies or drift in the class/convergence plan.

## Classification summary

| Class | Root dependencies |
|---|---|
| app-edge wiring | `clanker-router`, `clankers-agent-defs`, `clankers-artifacts`, `clankers-autoresearch`, `clankers-config`, `clankers-hooks`, `clankers-matrix`, `clankers-nix`, `clankers-plugin`, `clankers-prompts`, `clankers-runtime`, `clankers-skills`, `clankers-tts`, `clankers-ucan`, `clankers-zellij` |
| edge projection | `clanker-message`, `clanker-tui-types`, `clankers-protocol`, `clankers-tui` |
| adapter exception | `clankers-controller`, `clankers-procmon`, `clankers-tool-host` |
| temporary policy | `clankers-agent`, `clankers-db`, `clankers-model-selection`, `clankers-provider`, `clankers-session`, `clankers-util` |

## Budget evidence

The root dependency count remains at 28 after the already-completed `clankers-core` drain, but this slice narrows the remaining exception budget from prose-only owner categories into six explicit temporary-policy rows with convergence targets:

- agent construction must move behind runtime/controller adapters;
- DB storage access must move behind runtime/session stores;
- model-selection state must move behind provider/runtime service seams;
- provider construction/shaping must stay in the provider/router bridge;
- session storage policy must move behind SessionStore/ledger DTOs;
- reusable utility behavior must be replaced by owning brick APIs or edge-only helpers.

## Validation

Closeout validation for V1/V2 is recorded in `cairn/changes/shrink-root-shell-dependency-budget/evidence/closeout-validation.md`. The focused behavior tests, root thinking smoke, affected cargo check, architecture rails, Cairn tasks gate/validate, and `git diff --check` all passed.
