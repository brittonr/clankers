# Design: Enforce Workspace Layering Rails

## Layer model

Proposed default layers:

1. green contracts/core: `clanker-message`, `clanker-auth`, `clankers-engine`, extracted contract crates;
2. host/facade contracts: `clankers-engine-host`, `clankers-tool-host`, selected runtime host-injection contracts;
3. orchestration: `clankers-agent`, `clankers-controller`, provider compatibility adapters;
4. application shells: root CLI/TUI/daemon, plugin runtimes, transports, Matrix/ACP/MCP bridges.

Edges may point down or across explicitly allowed adapter seams. Edges pointing up require an owner receipt and should fail by default for green crates.

## Diagnostics

Failures should name the source crate/module, forbidden target layer, and suggested owner/adapter path. Prefer Cargo metadata and `syn` inventories over brittle exact source-string checks.
