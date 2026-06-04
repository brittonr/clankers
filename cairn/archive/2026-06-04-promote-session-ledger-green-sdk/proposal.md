# Change: Promote Session Ledger Green SDK

## Problem

Neutral session ledger DTOs and replay helpers currently live in `clankers-runtime`, which is documented as an embedding-facing facade but not a green generic SDK crate. The embedded session examples still use product-local DTOs instead of the reusable ledger, leaving the SDK story split between green engine-host crates and yellow runtime facade types.

## Goals

- Move or extract neutral `SessionLedgerEntry`/message/replay contracts into a green SDK crate or a new small green crate.
- Remove wall-clock construction and runtime-specific errors from the reusable ledger core.
- Update embedded session examples to dogfood the promoted ledger API.
- Keep desktop session storage and JSONL/DB compatibility at app-edge adapters.

## Non-goals

- Do not promote `clankers-session`, desktop JSONL storage, search indexes, or daemon resume logic into the generic SDK.
- Do not make runtime facade a mandatory dependency for minimal engine-host embedding.
- Do not rewrite all desktop session persistence in this slice.

## Proposed scope

Extract the pure ledger DTO/replay subset, adapt `clankers-runtime` and desktop session-ledger adapters to consume it, and replace product-local example history DTOs where they are duplicating the reusable shape.

## Verification

Focused validation should include session-resume brick fixtures, embedded session-store/workbench examples, dependency rails proving the green ledger crate excludes runtime/session/database/protocol crates, `scripts/check-session-ledger-boundary.rs`, `scripts/check-embedded-agent-sdk.rs`, Cairn gates, and `git diff --check`.
