Task-ID: I4
Covers: sdk-session-ledger-boundary.ledger-boundary.sdk-owned-store
Artifact-Type: documentation-evidence

# SDK Ledger Boundary Docs

## Summary

Updated `docs/src/tutorials/embedded-agent-sdk.md` to name `scripts/check-session-ledger-boundary.rs` beside the existing session-resume brick rail. The tutorial now states that product examples keep host-owned session/message DTOs while the daemon resume seed path projects desktop `AgentMessage` history through neutral `SessionLedgerEntry` DTOs at the desktop adapter edge.

## Existing example coverage

- `examples/embedded-session-store/src/main.rs` uses `ProductSession`, `ProductMessage`, and `InMemoryProductSessionStore`.
- `examples/embedded-product-workbench/src/main.rs` uses `ProductSession`, `ProductMessage`, and `ProductSessionStore`.
- `examples/embedded-session-store/session-resume-evidence.json` pins restored role/text ordering, missing-session behavior, and forbidden shell dependencies.

## Validation

`nix develop -c cargo -q -Zscript scripts/check-session-ledger-boundary.rs` checks the new tutorial marker plus SDK example forbidden-token boundaries.
