Evidence-ID: steel-tool-plugin-substrate.V1.checker
Task-ID: V1
Artifact-Type: deterministic-proof
Covers: steel-tool-plugin-substrate.substrate-contract.typed-plan, steel-tool-plugin-substrate.substrate-contract.no-ambient-authority, steel-tool-plugin-substrate.receipts.redaction, steel-tool-plugin-substrate.verification.boundary-rail
Created-By: pi
Created-At: 2026-05-29T00:00:00Z

# V1 Checker Evidence

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-steel-tool-plugin-substrate.rs
```

Result: passed.

Observed output:

```text
steel tool/plugin/subagent substrate receipt written to target/steel-tool-plugin-substrate/receipt.json
```

The checker validates runtime DTO/schema markers, agent dispatch adapter markers, receipt redaction markers, source-boundary ownership, executor-kind coverage, subagent/delegate backend tagging, and the Cairn spec/task anchors.
