# clanker-tui-types

Shared UI event, action, display, progress, panel, and plugin types for terminal agents and TUI frontends.

Workspace-local crate for [clankers](https://github.com/brittonr/clankers) TUI boundary types.

## Usage

```toml
[dependencies]
clanker-tui-types = { path = "../crates/clanker-tui-types" }
```

```rust
use clanker_tui_types::{PanelId, TuiEvent};

let panel_labels: Vec<&str> = PanelId::ALL.iter().map(|panel| panel.label()).collect();
let event = TuiEvent::AgentStart;

assert_eq!(panel_labels[0], "Todo");
assert!(matches!(event, TuiEvent::AgentStart));
```

## What lives here

- TUI action and keybinding-facing enums
- display, block, tool progress, and process snapshot types
- panel, selector, peer, and plugin UI metadata
- event enums shared across controller, tools, and TUI code
- thin aliases over `rat-leaderkey` and `rat-branches`

## Development

```bash
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --lib
```
