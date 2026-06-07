# Trait seam inventory and decision evidence

Evidence-ID: trait-seam-refactor-roadmap.trait-seam-inventory
Artifact-Type: design-inventory
Task-ID: I1,I2
Covers: remaining-coupling-drain.trait-seam-refactors.inventory, remaining-coupling-drain.trait-seam-refactors.justified-boundaries
Date: 2026-06-06
Status: PASS

## Inventory

| Candidate | Current implementations / branches | Existing tests / rails | Decision |
|-----------|------------------------------------|------------------------|----------|
| Plugin runtime lifecycle | `PluginKind::{Extism,Stdio,Zellij}`; Extism WASM instances and stdio supervisor/live-state fields were flat `PluginManager` fields; lifecycle methods branched on stdio | plugin runtime dispatch test; mixed-runtime plugin tests in crate | Traitify lifecycle now; keep manifest validation and summary projection outside runtime impls |
| OAuth provider flow | `OAuthFlow::{Anthropic,OpenAiCodex}` selected provider name, auth URL, code exchange, refresh; Codex endpoint/token/account claim logic separate helper functions | provider auth unit tests; Codex request/discovery tests | Traitify provider flow now while preserving enum public selection and provider-scoped credential store helpers |
| Framed session transport | Unix sockets already use `clankers_protocol::frame`; QUIC remote attach had private duplicate frame read/write helpers plus `QuicBiStream` adapter | local/remote attach reconnect tests; FCIS transport constructor rail | Reuse existing generic frame seam; remove QUIC duplicate helpers rather than inventing another transport trait |
| Session storage format | `.jsonl` vs `.automerge` extension checks lived in `SessionManager::open`, export, summary, import | session store tests; Automerge migration tests | Traitify format owner now for load/open/summary/import destination; leave file discovery preference helper as simple path selection |
| Process-job shell ports | pueue/systemd had duplicated Tokio `Command` execution/error projection; native/durable policy already under service/backend boundaries | pueue/systemd fake-runner tests; native durable/retention/notification tests | Introduce narrow shared command-runner shell port; keep backend policy and parsing in existing services |
| Passive DTOs and single-implementation helpers | Receipt/status/request/manifest structs and one-off helper constructors have no behavior polymorphism | covered by existing request/receipt tests | Keep/defer; do not traitify for style |

## Decision source

The decision table is also captured in `cairn/changes/trait-seam-refactor-roadmap/design.md` so future slices can see which boundaries were intentionally traitified, deferred, or kept as DTO/function ownership.
