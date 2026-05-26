# Tasks

- [x] [serial] T1. Design the local daemon attach/reconnect dogfood receipt schema and deterministic provider seam. [covers=r[daemon-attach-reconnect-dogfood.local-reconnect],r[daemon-attach-reconnect-dogfood.deterministic-provider],r[daemon-attach-reconnect-dogfood.cleanup-receipt]]
- [x] [parallel] T2. Implement the local socket daemon attach/reconnect harness with bounded tmux or process control. [covers=r[daemon-attach-reconnect-dogfood.local-reconnect],r[daemon-attach-reconnect-dogfood.parity-reset]]
- [x] [parallel] T3. Add cleanup assertions and receipt fields for daemon/session/process state. [covers=r[daemon-attach-reconnect-dogfood.cleanup-receipt]]
- [x] [serial] T4. Verify the focused dogfood rail and add it to an opt-in harness mode before considering full readiness promotion. [covers=r[daemon-attach-reconnect-dogfood.parity-reset],r[daemon-attach-reconnect-dogfood.deterministic-provider]]
