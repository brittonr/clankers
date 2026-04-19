## Why

Clankers has the storage layer for cross-session memory (`clankers-db/memory.rs`) and the system prompt injection (`Agent::system_prompt_with_memory`), but no way for the agent to write memories during a conversation. The `/memory` slash command lets the *user* manage memories from the TUI, but the agent itself has no tool to call. The agent also can't create skills from experience, search past sessions, or compress long contexts. These are table-stakes features for a coding agent that runs across many sessions on the same codebase.

Hermes Agent ships all four of these and they compound: the agent saves a memory → next session it loads that memory → it handles the task better → it saves the workflow as a skill → future sessions load the skill. Without the loop, each session starts from scratch.

## What Changes

- **Memory tool**: New `MemoryTool` the agent can call mid-conversation to add, replace, remove, and search memories. Capacity-bounded (configurable char limit) with usage reporting so the agent self-curates.
- **Skill management tool**: New `SkillManageTool` that lets the agent create, patch, and delete SKILL.md files in `~/.clankers/agent/skills/`. Triggered by system prompt guidance after complex tasks, error recovery, or user corrections.
- **Session search tool**: New `SessionSearchTool` that searches past session content via the existing `session_index` and JSONL files. Returns matching sessions with previews so the agent can recall prior work.
- **Context compression**: New `/compress` slash command and `CompressTool` that summarizes the current conversation via a cheap model, replacing the message history with the summary. Extends effective session length beyond mechanical truncation.
- **System prompt additions**: Guidance telling the agent when to save memories, when to create skills, and how to use session search. Capacity display in the memory section header.

## Capabilities

### New Capabilities
- `memory-tool`: Agent-callable tool for managing cross-session memory entries with capacity bounds
- `skill-manage-tool`: Agent-callable tool for creating and maintaining skills from experience
- `session-search-tool`: Agent-callable tool for searching past session content
- `context-compression`: LLM-based context summarization to extend session length

### Modified Capabilities

## Impact

- `src/tools/` — four new tool modules (`memory.rs`, `skill_manage.rs`, `session_search.rs`, `compress.rs`)
- `src/tools/mod.rs` — register new tools
- `src/modes/common.rs` — wire tools into `ToolSet` construction
- `crates/clankers-db/memory.rs` — add `total_chars()` method, capacity checking
- `crates/clankers-db/session_index.rs` — extend search to cover full session content (not just first prompt)
- `crates/clankers-agent/src/system_prompt.rs` — memory capacity header, learning loop guidance
- `crates/clankers-agent/src/lib.rs` — memory capacity in prompt injection
- `crates/clankers-agent/src/context.rs` — compression entry point
- `src/slash_commands/` — `/compress` handler
- `crates/clankers-config/src/settings.rs` — memory char limits, compression model config
