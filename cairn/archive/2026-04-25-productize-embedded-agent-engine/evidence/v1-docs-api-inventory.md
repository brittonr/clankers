Evidence-ID: v1-docs-api-inventory
Task-ID: V1
Artifact-Type: machine-check-log
Covers: embeddable-agent-engine.productized-sdk-surface.public-entrypoints-inventoried, embeddable-agent-engine.sdk-support-policy.inventory-classification, embeddable-agent-engine.embedding-api-stability-rails.public-api-inventory
Created: 2026-04-25T23:49:08Z
Status: pass

# V1 Docs/API inventory evidence

## Positive: current inventory maps docs to source

```text
ok: embedded SDK API inventory covers 110 public items (111 rows)
```

## Negative: stale source path fails

```text
source path for `EngineBufferedToolResult` does not exist: crates/clankers-engine/src/missing.rs
stale inventory failed as expected
```
