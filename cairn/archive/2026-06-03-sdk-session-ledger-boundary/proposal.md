# Change: Move Desktop Session Persistence Behind Neutral Ledger Boundaries

## Problem

Desktop session persistence still treats `AgentMessage` and `clankers-session` stores as canonical, while the SDK path has neutral ledger DTOs in runtime examples. Controller/root/session restore paths therefore remain coupled to Clankers transcript storage, DB/search, JSONL/automerge history, and display replay assumptions.

## Goals

- Make product-owned session ledger DTOs the reusable SDK storage boundary.
- Keep `clankers-session` as desktop compatibility storage until migrated.
- Convert persisted history to engine/semantic DTOs at adapter edges.
- Preserve restore/replay behavior for standalone, daemon attach, and embedded session examples.

## Non-goals

- Do not remove existing `.jsonl`/automerge compatibility in this slice.
- Do not require embedders to use Clankers session directories or message IDs.
- Do not rewrite merge/branch storage algorithms unless needed by the selected slice.

## Proposed scope

Inventory session persistence and replay paths, select one restore/resume path, and move it behind neutral ledger/session-store DTOs. Keep desktop store adapters responsible for `AgentMessage` compatibility and display replay conversion.

## Verification

Validation should include session-resume brick fixtures, runtime ledger tests, desktop restore parity tests, attach replay metadata parity, and source rails rejecting `clankers-session`/DB dependencies in green SDK examples.
