# Design: Daemon attach reconnect dogfood

## Approach

- Use local daemon/socket mode and deterministic provider/stub input so the rail proves attach/reconnect behavior rather than provider behavior.
- Exercise the existing attach parity tracker reset seam and a small set of visible state transitions.
- Capture bounded screen/log receipts under `target/dogfood/daemon-attach-reconnect-*`.
- Include cleanup assertions for daemon/session/process state.

## Verification Plan

- Run `nix run .#cairn -- validate --root .`.
- Run proposal/design/tasks gates for this change and inspect JSON validity/verdict.
- Run the focused implementation checks named in `tasks.md` when draining the change.
- Run `git diff --check` before commit.
