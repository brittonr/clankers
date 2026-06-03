# clanker-message

Stable content, tool-result, streaming, semantic-event, and Clankers transcript compatibility types for terminal coding agents.

Workspace-local crate for [clankers](https://github.com/brittonr/clankers) message boundary types.

## Usage

```toml
[dependencies]
clanker-message = { path = "../crates/clanker-message" }
```

```rust
use clanker_message::{Content, StopReason, Usage};

let content = Content::Text {
    text: "hello".to_string(),
};
let stop_reason = StopReason::Stop;
let usage = Usage::default();

assert!(matches!(content, Content::Text { .. }));
assert_eq!(stop_reason, StopReason::Stop);
assert_eq!(usage.total_tokens(), 0);
```

## What lives here

- stable typed content blocks (`Content`, `ImageSource`, `StopReason`)
- stable shared LLM contract structs (`Usage`, `ToolDefinition`, `ThinkingConfig`)
- router/provider-neutral streaming contracts and typed content events
- stable semantic session-event contracts (`SemanticEvent`, `SemanticEventMetadata`)
- tool result payloads and accumulation helpers
- Clankers transcript compatibility records under `transcript` (`AgentMessage`, `MessageId`, persisted timestamps, bash/custom/branch/compaction records)

`message::*` is a legacy compatibility module that re-exports both content and transcript types for existing callers. New embedded SDK code should prefer root exports or the `content`, `contracts`, `streaming`, `tool_result`, and `semantic_event` modules. Only Clankers session/provider/controller adapters should depend on `transcript` records.

## Development

```bash
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --lib
```
