## 1. Memory Tool

- [x] 1.1 Add `MemoryStore::total_chars(&self, scope: Option<&MemoryScope>) -> Result<usize>` to `crates/clankers-db/src/memory.rs` — sums `text.len()` for all entries in scope
- [x] 1.2 Add `MemoryLimits` struct and `memory.global_char_limit` / `memory.project_char_limit` fields to `crates/clankers-config/src/settings.rs` (defaults: 2200 / 1375)
- [x] 1.3 Create `src/tools/memory.rs` implementing `MemoryTool` with actions: add, replace, remove, search. Constructor takes `Db` + `MemoryLimits`. Capacity check on add (reject if over limit with current entries + usage in error). Substring matching for replace/remove (error on ambiguous match). All mutating actions return `usage: "N/M"` in response.
- [x] 1.4 Register `MemoryTool` in `src/tools/mod.rs` and wire it into `ToolSet` construction in `src/modes/common.rs`
- [x] 1.5 Update `Agent::system_prompt_with_memory()` in `crates/clankers-agent/src/lib.rs` to include capacity header: `MEMORY [67% — 1474/2200 chars]` above the entries
- [x] 1.6 Add memory usage guidance section to the system prompt (conditional on memory tool being in the active tool set) — when to save, when to skip
- [x] 1.7 Write tests for `MemoryTool`: add at capacity, replace with ambiguous match, remove nonexistent, search empty, capacity reporting

## 2. Skill Management Tool

- [x] 2.1 Create `src/tools/skill_manage.rs` implementing `SkillManageTool` with actions: create, patch, edit, delete, write_file, list. Constructor takes `PathBuf` (global skills dir).
- [x] 2.2 Validate skill names: lowercase alphanumeric + hyphens + underscores only. Validate `write_file` paths: no `..`, no absolute paths.
- [x] 2.3 `create`: write `SKILL.md` to `<global_skills_dir>/<name>/SKILL.md`, error if exists. `patch`: read file, exact string replace, write back. `edit`: overwrite entire file. `delete`: remove directory. `write_file`: write to `<global_skills_dir>/<name>/<file_path>`, mkdir -p parents. `list`: call `clankers_skills::scan_skills_dir` and return names + descriptions.
- [x] 2.4 Register `SkillManageTool` in `src/tools/mod.rs` and wire into `ToolSet` in `src/modes/common.rs`
- [x] 2.5 Add skill creation guidance to system prompt (conditional on skill_manage tool availability) — when to create skills, what makes a good skill
- [x] 2.6 Write tests for `SkillManageTool`: create + verify file exists, patch with missing old_text, delete nonexistent, path traversal rejection, list after create

## 3. Session Search Tool

- [x] 3.1 Create `src/tools/session_search.rs` implementing `SessionSearchTool`. Constructor takes `Db` + `PathBuf` (sessions dir) + `max_scan_files: usize`.
- [x] 3.2 Implement tier 1 search: call `SessionIndex::search()` on redb, return results with session ID, date, model, cwd, first_prompt
- [x] 3.3 Implement tier 2 JSONL scan: if tier 1 returns fewer than `limit` results, list JSONL files newest-first (by filename/mtime), open with `BufReader`, scan lines for query substring (case-insensitive), collect up to 3 matches per file with 1 line of surrounding context (200 char preview), stop after `max_scan_files`
- [x] 3.4 Add optional `cwd` filter parameter — when set, only return sessions matching that directory
- [x] 3.5 Register `SessionSearchTool` in `src/tools/mod.rs` and wire into `ToolSet` in `src/modes/common.rs`
- [x] 3.6 Write tests for `SessionSearchTool`: index-level search, JSONL scan fallback (create temp JSONL files), cwd filtering, result limit

## 4. Context Compression

- [x] 4.1 Add `CompressionSettings` to `crates/clankers-config/src/settings.rs`: `model` (optional string), `keep_recent` (default: 4), `min_messages` (default: 5)
- [x] 4.2 Create `src/tools/compress.rs` implementing `CompressTool`. Constructor takes `Arc<dyn Provider>` + `CompressionSettings`. The tool: collects messages 0..len-keep_recent, sends them to the compression model with a structured summary prompt, replaces old messages with a single summary User message prefixed `[Compressed context]`, returns before/after token counts and reduction percentage.
- [x] 4.3 Define the compression prompt: instruct the model to output sections for topics covered, decisions made, files touched, and open threads. Keep the prompt under 500 tokens.
- [x] 4.4 Implement the `/compress` slash command in `src/slash_commands/handlers/` — calls the same compression logic, shows before/after in TUI
- [x] 4.5 Add auto-nudge in `crates/clankers-agent/src/context.rs` `build_context()`: when estimated tokens exceed 80% of max_input_tokens, append a one-line advisory to the system prompt
- [x] 4.6 Register `CompressTool` in `src/tools/mod.rs` and wire into `ToolSet` in `src/modes/common.rs`
- [x] 4.7 Write tests for `CompressTool`: below min_messages rejection, recent message preservation, token count reporting. Test nudge insertion in `build_context` at threshold.

## 5. Integration and System Prompt

- [x] 5.1 Add all four tools to the default tool set in `src/modes/common.rs` (behind no feature gate — always available when Db is present)
- [x] 5.2 Verify tools work in daemon mode: `AgentProcess` passes `Db` through to tool construction. Confirm `DaemonToolRebuilder` includes new tools.
- [x] 5.3 Verify tools work in subagent mode: `run_ephemeral_agent` and `SubagentTool` include new tools when `Db` is available
- [x] 5.4 End-to-end test: start agent, save a memory, start a new session, verify memory appears in system prompt
- [x] 5.5 End-to-end test: start agent, create a skill, verify it appears in skill discovery on next session

## 6. Skill Usage Tracking

- [x] 6.1 Add `skill_usage` table to `crates/clankers-db/` — tracks per-skill load events with session ID, timestamp, and outcome (success/correction/failure)
- [x] 6.2 Add `record_skill_load` and `skill_stats` methods to the store
- [x] 6.3 Add `stats` action to `SkillManageTool` — shows load count, correction rate, last used date per skill
- [x] 6.4 Add system prompt guidance for skill self-review — "if a skill led you astray, update it"
- [x] 6.5 Write tests for skill usage tracking
