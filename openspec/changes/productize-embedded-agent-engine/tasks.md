## 1. SDK documentation and support policy

- [x] I1 Write the embedded-agent SDK guide at `docs/src/tutorials/embedded-agent-sdk.md`, covering the supported crate set, required host adapters, excluded Clankers shell concerns, adapter-only modular coupling, adapter recipes for model/tool/retry/event/cancel/usage/transcript conversion, support/versioning policy, migration-note location, and feature/default policy. [covers=embeddable-agent-engine.productized-sdk-surface,embeddable-agent-engine.productized-sdk-surface.supported-crate-set,embeddable-agent-engine.adapter-recipes,embeddable-agent-engine.adapter-recipes.positive-negative-paths,embeddable-agent-engine.adapter-recipes.transcript-conversion-owned-by-host,embeddable-agent-engine.adapter-only-modular-coupling,embeddable-agent-engine.adapter-only-modular-coupling.host-runner-traits,embeddable-agent-engine.adapter-only-modular-coupling.application-edge-composition,embeddable-agent-engine.sdk-support-policy,embeddable-agent-engine.sdk-support-policy.versioning-documented,embeddable-agent-engine.sdk-feature-default-policy,embeddable-agent-engine.sdk-feature-default-policy.documented] ✅ 1m (started: 2026-04-25T23:31:10Z → completed: 2026-04-25T23:32:30Z)
- [x] I2 Add a deterministic SDK public API inventory at `docs/src/generated/embedded-sdk-api.md` plus a checker that maps documented entrypoints and stability labels to actual exported Rust items or example paths. [covers=embeddable-agent-engine.productized-sdk-surface.public-entrypoints-inventoried,embeddable-agent-engine.sdk-support-policy.inventory-classification,embeddable-agent-engine.embedding-api-stability-rails,embeddable-agent-engine.embedding-api-stability-rails.public-api-inventory] ✅ 4m (started: 2026-04-25T23:34:40Z → completed: 2026-04-25T23:38:40Z)

## 2. External-consumer example

- [x] I3 Add `examples/embedded-agent-sdk/` as a standalone minimal consumer crate that submits an accepted prompt into `clankers-engine`, runs it through `clankers-engine-host::run_engine_turn`, and uses in-memory/fake model, tool, retry, event, cancellation, usage, and transcript-conversion adapters with both positive and negative paths. [covers=embeddable-agent-engine.external-consumer-example,embeddable-agent-engine.external-consumer-example.fake-adapters,embeddable-agent-engine.external-consumer-example.public-api-no-runtime-handles,embeddable-agent-engine.adapter-recipes.positive-negative-paths,embeddable-agent-engine.adapter-recipes.transcript-conversion-owned-by-host,embeddable-agent-engine.adapter-only-modular-coupling.host-runner-traits,embeddable-agent-engine.adapter-only-modular-coupling.application-edge-composition] ✅ 2m (started: 2026-04-25T23:39:00Z → completed: 2026-04-25T23:41:10Z)
- [ ] I4 Add manifest/dependency/feature checks proving the minimal example excludes `clankers-agent`, `clankers-controller`, `clankers-provider`, `clanker-router`, `clankers-db`, `clankers-protocol`, `clankers-tui`, `clankers-prompts`, `clankers-skills`, `clankers-config`, `clankers-agent-defs`, `ratatui`, `crossterm`, and `iroh`, and that documented default-feature/optional-feature expectations match Cargo manifests and a minimal example build. [covers=embeddable-agent-engine.external-consumer-example.dependency-graph-clean,embeddable-agent-engine.sdk-feature-default-policy.validated,embeddable-agent-engine.embedding-api-stability-rails.dependency-boundary-clean]

## 3. Validation rails and acceptance bundle

- [ ] I5 Extend `scripts/check-llm-contract-boundary.sh` and `crates/clankers-controller/tests/fcis_shell_boundaries.rs` or nearby focused rails so SDK crates and public contracts reject runtime shell, provider/router, daemon/TUI, database, networking, timestamp, shell-generated ID, runtime-handle, provider-shaped request/response leakage, hidden global service lookup, and direct dependencies on concrete Clankers runtime implementations from generic SDK crates. [covers=embeddable-agent-engine.external-consumer-example.public-api-no-runtime-handles,embeddable-agent-engine.embedding-api-stability-rails.dependency-boundary-clean,embeddable-agent-engine.adapter-only-modular-coupling.tight-coupling-rail]
- [ ] I6 Add `scripts/check-embedded-agent-sdk.sh` as the single documented acceptance bundle that runs the minimal example, docs/API reference checks, public API inventory checks, feature/default-policy checks, dependency/source boundary rails, generated artifact freshness checks, and focused Clankers host-runner parity checks. [covers=embeddable-agent-engine.embedding-acceptance-bundle,embeddable-agent-engine.embedding-acceptance-bundle.docs-examples,embeddable-agent-engine.embedding-acceptance-bundle.clankers-parity]

## 4. Verification and evidence

- [ ] V1 Positive/negative docs and API inventory verification: run the SDK docs/reference checker and public API inventory checker, proving documented entrypoints map to exported items/example paths, unsupported/internal items are not advertised as stable, and stale docs fail. [covers=embeddable-agent-engine.productized-sdk-surface.public-entrypoints-inventoried,embeddable-agent-engine.sdk-support-policy.inventory-classification,embeddable-agent-engine.embedding-api-stability-rails.public-api-inventory] [evidence=openspec/changes/productize-embedded-agent-engine/evidence/v1-docs-api-inventory.md]
- [ ] V2 Positive/negative example and dependency verification: run the minimal external-consumer fixture, its dependency denylist check, feature/default-policy check, and malformed/negative adapter paths, proving the example completes a turn through caller-supplied interfaces without forbidden Clankers shell dependencies, direct concrete runtime coupling, or required runtime-handle API leakage. [covers=embeddable-agent-engine.external-consumer-example.fake-adapters,embeddable-agent-engine.external-consumer-example.dependency-graph-clean,embeddable-agent-engine.external-consumer-example.public-api-no-runtime-handles,embeddable-agent-engine.sdk-feature-default-policy.validated,embeddable-agent-engine.adapter-recipes.positive-negative-paths,embeddable-agent-engine.adapter-only-modular-coupling.host-runner-traits,embeddable-agent-engine.adapter-only-modular-coupling.application-edge-composition] [evidence=openspec/changes/productize-embedded-agent-engine/evidence/v2-example-dependency-feature.md]
- [ ] V3 Positive Clankers parity verification: run focused `clankers-agent`/controller parity checks proving the default Clankers assembly still routes through the reusable host runner and preserves streaming deltas, tool execution and tool failures, retry/backoff behavior, cancellation behavior, usage observations/final summaries, and terminal stop/error behavior. [covers=embeddable-agent-engine.embedding-acceptance-bundle.clankers-parity] [evidence=openspec/changes/productize-embedded-agent-engine/evidence/v3-clankers-parity.md]
- [ ] V4 Final acceptance bundle verification: run `scripts/check-embedded-agent-sdk.sh`, capture its machine-produced output, and refresh generated docs/build artifacts so `docs/src/generated/embedded-sdk-api.md`, crate docs, and workspace metadata stay fresh. [covers=embeddable-agent-engine.embedding-acceptance-bundle,embeddable-agent-engine.embedding-acceptance-bundle.docs-examples,embeddable-agent-engine.embedding-api-stability-rails.dependency-boundary-clean] [evidence=openspec/changes/productize-embedded-agent-engine/evidence/v4-final-acceptance.md]

## Traceability Matrix

- `embeddable-agent-engine.productized-sdk-surface` -> `I1`
- `embeddable-agent-engine.productized-sdk-surface.supported-crate-set` -> `I1`
- `embeddable-agent-engine.productized-sdk-surface.public-entrypoints-inventoried` -> `I2`, `V1`
- `embeddable-agent-engine.external-consumer-example` -> `I3`
- `embeddable-agent-engine.external-consumer-example.fake-adapters` -> `I3`, `V2`
- `embeddable-agent-engine.external-consumer-example.dependency-graph-clean` -> `I4`, `V2`
- `embeddable-agent-engine.external-consumer-example.public-api-no-runtime-handles` -> `I3`, `I5`, `V2`
- `embeddable-agent-engine.adapter-recipes` -> `I1`
- `embeddable-agent-engine.adapter-recipes.positive-negative-paths` -> `I1`, `I3`, `V2`
- `embeddable-agent-engine.adapter-recipes.transcript-conversion-owned-by-host` -> `I1`, `I3`
- `embeddable-agent-engine.adapter-only-modular-coupling` -> `I1`
- `embeddable-agent-engine.adapter-only-modular-coupling.host-runner-traits` -> `I1`, `I3`, `V2`
- `embeddable-agent-engine.adapter-only-modular-coupling.application-edge-composition` -> `I1`, `I3`, `V2`
- `embeddable-agent-engine.adapter-only-modular-coupling.tight-coupling-rail` -> `I5`
- `embeddable-agent-engine.sdk-support-policy` -> `I1`
- `embeddable-agent-engine.sdk-support-policy.versioning-documented` -> `I1`
- `embeddable-agent-engine.sdk-support-policy.inventory-classification` -> `I2`, `V1`
- `embeddable-agent-engine.sdk-feature-default-policy` -> `I1`
- `embeddable-agent-engine.sdk-feature-default-policy.documented` -> `I1`
- `embeddable-agent-engine.sdk-feature-default-policy.validated` -> `I4`, `V2`
- `embeddable-agent-engine.embedding-api-stability-rails` -> `I2`
- `embeddable-agent-engine.embedding-api-stability-rails.public-api-inventory` -> `I2`, `V1`
- `embeddable-agent-engine.embedding-api-stability-rails.dependency-boundary-clean` -> `I4`, `I5`, `V4`
- `embeddable-agent-engine.embedding-acceptance-bundle` -> `I6`, `V4`
- `embeddable-agent-engine.embedding-acceptance-bundle.docs-examples` -> `I6`, `V4`
- `embeddable-agent-engine.embedding-acceptance-bundle.clankers-parity` -> `I6`, `V3`
