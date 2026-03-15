# Capabilities

## Purpose

Define the capability types that scope what a token holder can do when
interacting with the clankers daemon.  These replace aspen's KV/secrets
capabilities with operations meaningful to a coding agent.

## Requirements

### Capability enum

The system MUST define these capability variants:

```rust
enum Capability {
    /// Can send prompts to the agent.  This is the base capability —
    /// without it, the token holder can't do anything.
    Prompt,

    /// Can use specific tools.  Pattern supports:
    /// - Exact: "read"
    /// - Glob: "grep,find,ls,read" (comma-separated list)
    /// - Wildcard: "*" (all tools)
    ToolUse {
        /// Comma-separated tool names or "*"
        tool_pattern: String,
    },

    /// Can execute shell commands via the bash tool.
    /// Inherits aspen's ShellExecute semantics — glob patterns on
    /// command names and optional working directory constraint.
    ShellExecute {
        command_pattern: String,
        working_dir: Option<String>,
    },

    /// Can access files matching a path prefix.
    /// Scopes read/write/edit tool operations.
    FileAccess {
        /// Path prefix (e.g., "/home/user/project/")
        prefix: String,
        /// Read-only or read-write
        read_only: bool,
    },

    /// Can use bot commands.  Pattern is comma-separated command names
    /// or "*" for all.
    BotCommand {
        /// e.g., "status,skills,help" or "*"
        command_pattern: String,
    },

    /// Can manage sessions (restart, compact).
    SessionManage,

    /// Can switch the model.
    ModelSwitch,

    /// Can create child tokens with attenuated capabilities.
    Delegate,
}
```

### Capability authorization

Each capability MUST implement an `authorizes(&self, op: &Operation) -> bool`
method following the same pattern as aspen-auth.

GIVEN a token with `ToolUse { tool_pattern: "read,grep,find" }`
WHEN the agent attempts to use the `bash` tool
THEN the operation is denied

GIVEN a token with `ToolUse { tool_pattern: "*" }`
WHEN the agent attempts to use any tool
THEN the operation is authorized

GIVEN a token with `FileAccess { prefix: "/home/user/project/", read_only: true }`
WHEN the agent attempts to write to `/home/user/project/src/main.rs`
THEN the operation is denied (read_only)

GIVEN a token with `FileAccess { prefix: "/home/user/project/", read_only: false }`
WHEN the agent attempts to write to `/etc/passwd`
THEN the operation is denied (outside prefix)

### Capability containment (for delegation)

Each capability MUST implement `contains(&self, other: &Capability) -> bool`
for delegation chain validation.

- `ToolUse { "*" }` contains `ToolUse { "read,grep" }`
- `FileAccess { prefix: "/home/", read_only: false }` contains
  `FileAccess { prefix: "/home/user/", read_only: true }`
- `FileAccess { prefix: "/home/", read_only: true }` does NOT contain
  `FileAccess { prefix: "/home/user/", read_only: false }` (can't escalate to write)
- `BotCommand { "*" }` contains `BotCommand { "status,help" }`
- `SessionManage` contains only itself
- `Delegate` contains only itself

### Operation enum

```rust
enum Operation {
    /// User sends a prompt
    Prompt { text: String },
    /// Agent uses a tool
    ToolUse { tool_name: String },
    /// Agent executes a shell command
    ShellExecute { command: String, working_dir: Option<String> },
    /// Agent accesses a file
    FileRead { path: String },
    FileWrite { path: String },
    /// User sends a bot command
    BotCommand { command: String },
    /// User manages session
    SessionManage { action: String },
    /// User switches model
    ModelSwitch { model: String },
}
```

### Default root token

The daemon owner's root token MUST include all capabilities:

```rust
TokenBuilder::new(secret_key)
    .with_capability(Capability::Prompt)
    .with_capability(Capability::ToolUse { tool_pattern: "*".into() })
    .with_capability(Capability::ShellExecute { command_pattern: "*".into(), working_dir: None })
    .with_capability(Capability::FileAccess { prefix: "/".into(), read_only: false })
    .with_capability(Capability::BotCommand { command_pattern: "*".into() })
    .with_capability(Capability::SessionManage)
    .with_capability(Capability::ModelSwitch)
    .with_capability(Capability::Delegate)
    .with_lifetime(Duration::from_secs(86400 * 365))
    .build()?
```
