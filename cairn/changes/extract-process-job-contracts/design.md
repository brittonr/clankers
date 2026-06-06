# Design: Extract Process Job Contracts

## Boundary

The green owner defines typed requests, decisions, stable receipt DTOs, redaction metadata, and backend capability descriptors. It must not depend on root CLI/TUI modules, daemon actors, procmon implementations, pueue/systemd command execution, global config paths, or filesystem storage.

## Adapter shape

Root and backend code keep the imperative shell:

1. parse JSON/tool input into the green request DTO;
2. call a process-job service/backend adapter;
3. project the typed receipt to agent-visible tool output or UI events.

Backend-specific rules stay behind typed backend traits so native, pueue, systemd, and future backends can diverge without reintroducing inline policy in `src/tools/process.rs`.

## Rails

The slice should update source-boundary rails to fail when process-job policy appears in root projection modules, and should refresh runtime facade inventory if DTOs leave `clankers-runtime`.
