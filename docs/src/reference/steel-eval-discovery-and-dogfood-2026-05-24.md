# Steel Eval Discovery and Controlled Dogfood Evidence 2026-05-24

This note records the local, secret-free evidence surfaces for the default `steel_eval` tool.

## Live tool discovery receipt

Run:

```bash
./scripts/check-steel-eval-tool-discovery.rs
```

Receipt path:

```text
target/steel-eval/tool-discovery/receipt.json
```

The receipt proves the product-level runtime discovery path `build_tiered_tools(ToolEnv { settings: Some(Settings::default()), .. })` publishes `steel_eval` by default, hides it when `steelEval.enabled = false`, and hides it through the standard disabled-tool policy. The receipt is metadata-only: it records source hashes, safe assertions, and the pure default authority boundary; it does not execute Steel source, expose credentials, or perform mutation.

## Controlled corpus dogfood receipt

Run:

```bash
./scripts/check-steel-eval-controlled-corpus.rs
```

Inputs and output:

```text
policy/steel-eval/controlled-corpus.json
target/steel-eval/controlled-corpus/receipt.json
```

The corpus manifest is local and checked in. It defines the corpus id, threshold policy, minimum case count, maximum regression budget, and the pure default authority boundary. The dogfood receipt records only case ids, source/output hashes, status/reason classes, and redaction metadata. Raw source and raw output are omitted from per-case receipts.

Current outcome from the local corpus is `pass`: five cases matched, zero regressions, success rate `1.0`, and recommendation `true`. The corpus includes positive evaluation cases and negative classification cases for ambient authority denial and unsupported expression handling.

## Authority boundary

Both receipts keep `steel_eval` inside the pure wrapper boundary:

- host functions: `[]`
- session capabilities: `[]`
- max host calls: `0`
- ambient authority: `false`
- network/filesystem/process/mutation/credentials: denied

The dogfood path does not grant mutation, remote fetch, credentials, or default host authority. Any future non-pure profile must be a separate reviewed configuration and cannot inherit authority from these receipts.
