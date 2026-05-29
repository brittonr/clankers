# Proposal: Observable Soak Rails

## Problem

Clankers is finding important interaction bugs only during manual pi dogfood: token rendering can appear stalled, attach-mode input can be accepted by the UI but blocked by the daemon actor, and reconnect/process-panel paths need repeated observation to expose flakes. Today the individual rails exist or are being added, but there is no Cairn package that defines the finish line for pi-observable Clankers behavior or a maintained harness surface for repeated soak runs.

## Proposed Change

Add a Cairn change that defines the observable-soak finish line for Clankers interaction stability. The change requires dogfood receipts for standalone streaming, daemon/attach streaming abort and follow-up, daemon attach reconnect, and background-process panel visibility. It also requires a `./scripts/test-harness.sh soak ...` mode that repeats those rails with stable receipts so pi can observe all critical product surfaces through deterministic local stubs.

## Impact

- Pi and human operators get one named soak harness surface instead of ad hoc repeated commands.
- Streaming, attach abort, reconnect, process-panel, cleanup, and receipt fields become explicit requirements.
- The current work remains local/deterministic and does not claim unattended public production readiness.
