## Phase 1: Ledger contract

- [ ] [serial] I1: Define neutral session ledger DTOs and traits for session identity, prompt turns, assistant/tool content, summaries, usage, and safe receipts. [covers=r[session-resume-brick.ledger-contract.neutral-entries]]
- [ ] [serial] I2: Add conversion from ledger replay entries to `EngineMessage` history and host-facing metadata without importing `AgentMessage`, daemon frames, TUI blocks, or database row types in the reusable API. [covers=r[session-resume-brick.replay-contract.engine-history]]
- [ ] [parallel] I3: Implement in-memory/product-owned store adapters and desktop adapters for existing session storage without making desktop storage a generic SDK dependency. [covers=r[session-resume-brick.storage-adapters.host-owned]]
- [ ] [serial] I4: Wire runtime session resume through the ledger contract with explicit stateless vs resume-required policy. [covers=r[session-resume-brick.runtime-integration.fail-closed-missing-session]]

## Phase 2: Verification

- [ ] [parallel] V1: Add two-backend resume fixtures proving restored user/assistant/tool/summary context reaches the next `EngineModelRequest` in stable order. [covers=r[session-resume-brick.verification.two-backend-resume]]
- [ ] [parallel] V2: Add missing-session and unsupported-store fail-closed tests that prove no model/tool execution occurs. [covers=r[session-resume-brick.verification.fail-closed]]
- [ ] [serial] V3: Run session/runtime/controller focused tests, embedded session-store checks, Cairn validate/gates, and `git diff --check`. [covers=r[session-resume-brick.verification.closeout]]
