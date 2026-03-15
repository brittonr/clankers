# Spec: Derive Tool Collision List

## Overview

The `builtin_names` HashSet in `build_plugin_tools()` is manually maintained
and already out of sync with the actual tool list. Replace it with a derived
set.

## Current Code

```rust
// src/modes/common.rs, build_plugin_tools()
let builtin_names: HashSet<&str> = [
    "read", "write", "edit", "bash", "grep", "find", "ls",
    "subagent", "delegate_task", "todo", "nix", "web",
    "commit", "review", "ask", "image_gen", "validate_tui", "procmon",
].into_iter().collect();
```

This set is not derived from `build_tools_with_events()`. If a tool is added
or renamed, this list must be updated manually. Forgetting causes silent name
collisions where a plugin tool shadows a builtin.

## Fix

Pass the actual tool list to `build_plugin_tools()`:

```rust
pub fn build_plugin_tools(
    builtin_tools: &[Arc<dyn Tool>],
    plugin_manager: &PluginManager,
) -> Vec<Arc<dyn Tool>> {
    let builtin_names: HashSet<&str> = builtin_tools
        .iter()
        .map(|t| t.name())
        .collect();

    // ... rest unchanged, uses builtin_names for collision check
}
```

At the call site in `interactive.rs`:

```rust
let tools = build_tools_with_events(...);
let plugin_tools = build_plugin_tools(&tools, &plugin_manager);
```

## Scope

One function signature change, one call-site update, delete the hardcoded
array. No trait needed.
