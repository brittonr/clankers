## MODIFIED Requirements

### Requirement: Named capability-pack presets for embeddings [r[embedded-composition-kits.capability-packs]]

The system MUST provide safe named capability-pack presets that embedders can select and test deterministically.

#### Scenario: Safe presets do not expand unexpectedly [r[embedded-composition-kits.capability-packs.no-expansion]]

- GIVEN product-facing presets named `embedding_safe`, `read_only`, `networkless_coding`, `project_local_edit`, and `human_approved_shell`
- WHEN each preset is converted into its ordered embedded capability set
- THEN focused tests MUST assert the exact allowed capability set for every preset
- THEN `embedding_safe`, `read_only`, and `networkless_coding` MUST NOT include explicit opt-in capabilities such as mutate, shell, network, raw-log, or secret-adjacent access unless the expected snapshot and docs are intentionally updated
- THEN adding a dangerous capability to a safe preset MUST fail a focused regression test unless the change explicitly updates docs and expected evidence

#### Scenario: Dangerous packs require explicit opt-in [r[embedded-composition-kits.capability-packs.explicit-danger]]

- GIVEN a capability pack can mutate files, run shell/process work, access network, expose raw logs, or work near secrets
- WHEN a product selects that pack
- THEN the API and docs MUST make the risk explicit through the preset name or description
- THEN `human_approved_shell` MUST be treated as an explicit opt-in pack rather than a default minimal embedding preset
- THEN the default minimal embedding path MUST NOT select that pack implicitly

### Requirement: Embedded composition acceptance rail [r[embedded-composition-kits.acceptance-rail]]

The system MUST extend the existing embedded SDK acceptance command so lego-style composition claims are verified before readiness is claimed.

#### Scenario: One command verifies lego readiness [r[embedded-composition-kits.acceptance-rail.one-command]]

- GIVEN a developer changes adapter bricks, kits, catalogs, capability packs, provider/session recipes, or embedded SDK docs
- WHEN `scripts/check-embedded-agent-sdk.sh` runs
- THEN it MUST verify API inventory freshness, dependency denylist coverage, source-boundary checks, executable recipes, catalog negative cases, capability-pack snapshots, host-owned session-store recipe behavior, and focused engine/host/tool parity tests
- THEN failure MUST identify the violated lego-boundary rule with enough detail to fix the offending dependency, source token, catalog field, capability-pack preset, session-store assertion, or recipe assertion
