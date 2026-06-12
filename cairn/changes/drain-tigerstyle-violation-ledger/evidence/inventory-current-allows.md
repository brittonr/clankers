Artifact-Type: inventory
Task-ID: R#inventory-current-allows
Covers: tigerstyle-compliance.violation-ledger

# Inventory: Current Tigerstyle Allow Sites

Command:

```bash
rg 'tigerstyle::[a-zA-Z0-9_]+' src crates plugins --glob '*.rs'
```

Result: the current inventory is recorded in `cairn/changes/drain-tigerstyle-violation-ledger/design.md` under `## Violation Inventory`. The inventory excludes historical `.git/clankers-checkpoints` snapshots and includes source allow sites under `src/` and `crates/`.
