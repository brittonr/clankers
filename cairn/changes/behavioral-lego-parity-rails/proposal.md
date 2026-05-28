# Proposal: Behavioral Lego Parity Rails

## Problem

Several current lego/SDK acceptance scripts are freshness or symbol-presence checks. They assert that names like matrix cases, runtime service tests, or shell adapter fixtures exist, but they do not always execute a behavioral fixture that proves real cross-shell parity. This lets architecture docs claim Lego readiness while important behavior can drift.

## Proposed Change

Replace string-presence rails with executable, receipt-backed behavioral rails for runtime extension services, shell adapter parity, semantic events, provider services, session resume, tool contexts, and root/controller dependency ownership. The rails should emit durable evidence naming cases, axes, expected outcomes, and observed outcomes.

## Impact

- **Files**: `scripts/check-*-matrix.rs`, `scripts/check-embedded-agent-sdk.rs`, focused tests in runtime/agent/controller/provider/tool/session crates, receipt artifacts under `target/embedded-sdk-release`.
- **Testing**: actual fixture execution, deterministic receipts, negative mutation/fail-closed fixtures, CI/Nix wiring.
