## Phase 1: API foundation

- [x] [serial] Write the embeddable runtime API OpenSpec package. [covers=embeddable-runtime-api.facade] [evidence=openspec validate define-embeddable-runtime-api --strict]
- [ ] [serial] Add the runtime facade crate/module with `RuntimeBuilder`, `SessionHandle`, prompt input, control methods, and typed event stream. [covers=embeddable-runtime-api.facade]
- [ ] [parallel] Add public API boundary tests that reject daemon/TUI/ACP/MCP/CLI type leakage. [covers=embeddable-runtime-api.adapter-parity.no-leakage]
- [ ] [parallel] Add fake-provider prompt tests for host-facing event ordering and safe metadata. [covers=embeddable-runtime-api.events.prompt-stream]

## Phase 2: Adapter convergence

- [ ] [serial] Wire one existing headless or daemon path through the runtime facade, or add a parity harness proving identical semantics. [covers=embeddable-runtime-api.adapter-parity.prompt]
- [ ] [parallel] Document the supported Rust embedding API and current non-goals. [covers=embeddable-runtime-api.facade.create-session]
