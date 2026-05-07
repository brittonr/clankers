## Phase 1: API foundation

- [x] [serial] Write the embeddable runtime API OpenSpec package. [covers=embeddable-runtime-api.facade] [evidence=openspec validate define-embeddable-runtime-api --strict]
- [x] [serial] Add the runtime facade crate/module with `RuntimeBuilder`, `SessionHandle`, prompt input, control methods, and typed event stream. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-api.facade] [evidence=CARGO_TARGET_DIR=target cargo test -p clankers-runtime]
- [x] [parallel] Add public API boundary tests that reject daemon/TUI/ACP/MCP/CLI type leakage. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-api.adapter-parity.no-leakage] [evidence=clankers-runtime::tests::public_api_boundary_rejects_transport_type_leakage]
- [x] [parallel] Add fake-provider prompt tests for host-facing event ordering and safe metadata. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-api.events.prompt-stream] [evidence=clankers-runtime::tests::runtime_facade_streams_host_events_in_order]

## Phase 2: Adapter convergence

- [x] [serial] Wire one existing headless or daemon path through the runtime facade, or add a parity harness proving identical semantics against a real existing path. ✅ (completed: 2026-05-07T03:52:50Z) [covers=embeddable-runtime-api.adapter-parity.prompt] [evidence=cargo test -p clankers runtime_facade_batch_executor_matches_existing_batch_semantics]
- [x] [parallel] Document the supported Rust embedding API and current non-goals. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=embeddable-runtime-api.facade.create-session] [evidence=docs/src/reference/embedding.md]
