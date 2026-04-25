# clanker-message

Conversation message, content, tool-result, and streaming types for terminal coding agents.

Workspace-local crate for [clankers](https://github.com/brittonr/clankers) message boundary types.

## Usage

```toml
[dependencies]
clanker-message = { path = "../crates/clanker-message" }
```

```rust
use clanker_message::{AgentMessage, Content, MessageId, UserMessage};
use chrono::Utc;

let message = AgentMessage::User(UserMessage {
    id: MessageId::new("u1"),
    content: vec![Content::Text {
        text: "hello".to_string(),
    }],
    timestamp: Utc::now(),
});

assert!(message.is_user());
assert_eq!(message.role(), "user");
```

## What lives here

- conversation message enums and structs (`AgentMessage`, `UserMessage`, `AssistantMessage`)
- typed content blocks (`Content`, `ImageSource`, `StopReason`)
- tool result payloads and accumulation helpers
- router/provider-neutral streaming contracts and typed content events
- shared LLM contract structs (`Usage`, `ToolDefinition`, `ThinkingConfig`)
- shared message IDs and random ID generation helpers

## Development

```bash
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --lib
```
