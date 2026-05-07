## Why

The reusable SDK path is decoupled from CLI/TUI/daemon shells, but existing Clankers shells still need to preserve behavior while acting as adapters. Current FCIS rails check source boundaries and focused parity seams; they do not yet define a combined shell-adapter matrix across prompt assembly, stores, confirmation, tools, model execution, and event translation.

## What Changes

- Add a shell-adapter parity matrix for standalone agent, controller/daemon, attach/TUI, and batch/embedded entrypoints where bounded.
- Combine host-owned stores, prompt assembly, confirmation broker decisions, tool filtering, model/provider adapter output, and event translation.
- Keep live/provider/network cases out of the generic matrix; use fake adapters and recorded shell fixtures.

## Capabilities

### Modified Capabilities
- `embeddable-agent-engine`: shell adapter parity covers combined runtime features without moving policy back into shells.
- `prompt-assembly`: prompt assembly participates in adapter parity as a host-owned service input, not an engine dependency.

## Impact

- **Files**: FCIS shell boundary tests, controller/agent adapter tests, prompt/store/confirmation fixtures, acceptance scripts.
- **APIs**: no public API changes intended.
- **Testing**: focused adapter parity matrix plus existing FCIS shell-boundary tests.
