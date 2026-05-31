Evidence-ID: harden-steel-substrate-checker-paths.V1.checker
Task-ID: V1
Artifact-Type: command-output
Covers: steel-tool-plugin-substrate.checker-paths.active-archive-resolution, steel-tool-plugin-substrate.checker-paths.receipt-artifacts
Status: pass
Generated-By: pi
Generated-At: 2026-05-30

# Focused Checker Evidence

## Command

```text
./scripts/check-steel-tool-plugin-substrate.rs
```

## Output

```text
steel tool/plugin/subagent substrate receipt written to target/steel-tool-plugin-substrate/receipt.json
```

## Receipt path proof

The generated receipt hashes the resolved archived task ledger and canonical specification paths:

```json
{
  "hashed_artifacts": [
    { "path": "cairn/archive/1970-01-01-steel-tool-plugin-substrate/tasks.md" },
    { "path": "cairn/specs/steel-tool-plugin-substrate/spec.md" }
  ]
}
```

The checker therefore no longer requires the absent active path `cairn/changes/steel-tool-plugin-substrate/tasks.md`.
