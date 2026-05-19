# Embedded SDK Lego Checkpoint — 2026-05-19

This checkpoint records the embedded SDK lego/brick readiness slice completed on `main`.

## Checkpoint

- Branch: `main`
- Tag: `embedded-sdk-lego-2026-05-19`
- Tagged checkpoint commit: the commit carrying this note
- Embedded SDK lego payload commit: `02063512066d577c61f5a9a5b2f7b1d0d6c5b9a2`
- Release receipt: `target/embedded-sdk-release/receipt.json`
- Receipt status at generation: `## main...origin/main`
- Receipt artifact count: 49 hashed artifacts

## What landed

The OpenSpec queue for embedded SDK lego/brick readiness was drained and archived. The completed rails cover:

- real product dogfood evidence
- provider adapter kit fixtures
- session/resume brick convergence
- declarative tool catalog manifests
- capability-pack composition policy
- plugin/tool runtime dispatch separation
- brick inventory stability and migration-note semantics
- prompt assembly kit
- confirmation broker kit
- batch eval runner kit
- slash command routing kit
- TUI action/menu kit
- daemon event translation kit
- controller continuation policy kit
- observability audit receipt kit
- self-evolution receipt-chain kit
- process job profile kit

## Receipts and evidence

The checkpoint receipt set includes these generated artifacts under `target/embedded-sdk-release/`:

- `receipt.json`
- `lego-contracts-receipt.json`
- `product-dogfood/receipt.json`
- `provider-adapter-kit-receipt.json`
- `session-resume-brick-receipt.json`
- `tool-catalog-manifest-receipt.json`
- `capability-pack-composition-receipt.json`
- `plugin-runtime-dispatch-receipt.json`
- `brick-inventory-stability-receipt.json`

The receipt generator reported commit `02063512`, clean `main...origin/main`, and 49 hashed artifacts.

## Verification run

Release-readiness checkpoint commands run after the OpenSpec drain:

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --workspace --all-targets
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/verify.sh
./scripts/emit-embedded-sdk-release-receipt.rs --output target/embedded-sdk-release/receipt.json
```

Observed verification evidence:

- workspace `cargo check --workspace --all-targets` completed without surfaced failure
- `./scripts/verify.sh` exited `0`
- embedded controller parity suite: 32 tests run, 32 passed, 0 skipped
- no-std functional core validation bundle passed
- Tracey coverage: 47 of 47 requirements covered, 47 of 47 have verification references
- final verify output: `=== All checks passed ===`

## Scope and caveats

This checkpoint supports trusted/internal dogfooding and embedded SDK lego/brick readiness. It does not by itself claim unattended public production readiness. Broader release confidence can add the host-dependent readiness gates from [Release Readiness](./release-readiness.md), including live local-model, VM, and flake-heavy checks on authorized machines.
