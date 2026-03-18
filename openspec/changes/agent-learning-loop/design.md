## Context

Clankers has a layered architecture: `clankers-agent` owns the core loop, `clankers-db` provides persistent storage (redb), tools live in `src/tools/`, and the system prompt is assembled in `clankers-agent/src/system_prompt.rs` with memory injected in `Agent::system_prompt_with_memory()`.

Current state:
- **Memory**: `clankers-db/memory.rs` has `MemoryStore` with save/remove/update/search/list/context_for. The `/memory` slash command exposes these to the user. The agent has no tool to call them.
- **Skills**: `clankers-skills` scans `~/.clankers/agent/skills/*/SKILL.md` and project-local `.clankers/skills/*/SKILL.md`. Read-only discovery — no write path.
- **Session index**: `clankers-db/session_index.rs` indexes sessions by first prompt, model, cwd, session ID. Substring search only on metadata, not full content.
- **Context management**: `clankers-agent/src/context.rs` does mechanical truncation (drop middle messages) and tool result compaction (replace old results with `[tool: N lines, M bytes]`). No LLM-based summarization.

All four tools follow the existing `Tool` trait pattern: struct implementing `async fn run(&self, input: Value, ctx: ToolContext) -> ToolResult`. They receive a `Db` handle via their struct fields (same pattern as `CostTool`, `TodoTool`).

## Goals / Non-Goals

**Goals:**
- Let the agent manage its own memory without user intervention
- Let the agent create skills from experience so future sessions benefit
- Give the agent recall across sessions via search
- Extend effective session length via LLM summarization
- All four tools work in standalone mode, daemon mode, and subagent mode

**Non-Goals:**
- Automatic memory extraction at session end (can be added later; start with explicit tool calls)
- Skills Hub or remote skill registry (manual authoring and agent creation are sufficient)
- Full-text indexing of all session content at write time (too much storage overhead; scan on demand)
- Automatic compression (advisory nudge only; the agent or user decides when)
- Memory deduplication or embedding-based semantic search (substring matching is good enough to start)

## Decisions

### 1. Memory tool uses Db handle directly, not slash command forwarding

The `MemoryTool` struct holds an `Arc<Db>` (or `Option<Db>`) and calls `MemoryStore` methods directly. No message passing through the TUI.

*Alternative*: Route through slash command handlers via `AgentCommand` enum. Rejected because tools run in the agent loop, which already has access to the Db. Adding a round-trip through the TUI channel would be slower and more complex.

### 2. Capacity enforcement lives in the tool, not the store

`MemoryStore` stays unchanged — it's a dumb CRUD layer. The `MemoryTool` checks `total_chars()` against the configured limit before calling `save()`. This keeps the DB layer simple and testable while letting the tool return rich error messages with current entries and usage.

A new `MemoryStore::total_chars(scope)` method sums entry text lengths for a given scope. The tool computes `current + new_entry.len()` and rejects if over limit.

### 3. Skill manage tool writes to filesystem, not a database

Skills are SKILL.md files on disk, discovered by `clankers-skills::scan_skills_dir`. The `SkillManageTool` writes files directly — no DB intermediary. This keeps skills human-readable and version-controllable (users can `git add` their skills directory).

The tool only writes to the global skills directory (`~/.clankers/agent/skills/`). Project-local skills remain manually authored. This prevents the agent from polluting project repos.

### 4. Session search uses a two-tier strategy

Tier 1: Search `session_index` in redb (fast, covers metadata). Tier 2: If tier 1 returns fewer results than requested, scan JSONL files with `grep`-style line matching (slower, covers full content).

*Alternative*: Build a full-text index over all session content at write time. Rejected — redb doesn't have FTS, adding SQLite just for this is overkill, and the JSONL scan bounded to 100 files is fast enough for the expected corpus size.

The JSONL scan reads files newest-first, opens each with `BufReader`, scans lines for the query substring, and collects matches with surrounding context. A configurable `max_scan_files` (default: 100) caps I/O.

### 5. Context compression is a tool + slash command, not automatic

Compression runs on demand: the agent calls `compress` or the user types `/compress`. An advisory nudge at 80% context capacity reminds the agent it's available, but the system never compresses automatically.

*Alternative*: Auto-compress at a threshold. Rejected because compression is lossy — the agent should decide what to keep based on the current task. Automatic compression during a debugging session could drop critical error context.

### 6. Compression model selection

The compression prompt is sent to the cheapest available model from the active provider. A `compression.model` setting overrides this. The prompt asks for structured output: topics, decisions, files, open threads.

The compressed summary replaces messages 0..N-keep_recent with a single `AgentMessage::User` containing the summary text prefixed with `[Compressed context from earlier in this session]`. Using a User message (not system) keeps the system prompt stable for prefix caching.

### 7. All tools receive Db and config via constructor injection

```
pub struct MemoryTool { db: Db, limits: MemoryLimits }
pub struct SkillManageTool { global_skills_dir: PathBuf }
pub struct SessionSearchTool { db: Db, sessions_dir: PathBuf, max_scan: usize }
pub struct CompressTool { provider: Arc<dyn Provider>, settings: CompressionSettings }
```

Constructed in `src/modes/common.rs` alongside existing tools. The `CompressTool` needs a provider handle to make the summarization call — it receives the same `Arc<dyn Provider>` used by the agent.

### 8. System prompt additions are conditional on tool availability

The memory usage guidance paragraph is only injected when the `memory` tool is in the active tool set. Same for skill creation guidance and `skill_manage`. This avoids confusing the agent about capabilities it doesn't have (e.g., in a restricted subagent with limited tools).

## Risks / Trade-offs

**[Unbounded JSONL scan I/O]** → Mitigated by `max_scan_files` cap (default: 100). Worst case: scanning 100 × ~1MB files = ~100MB I/O. Acceptable for an on-demand tool. Can add async I/O later if needed.

**[Compression quality varies by model]** → Mitigated by structured prompt that constrains output format. Haiku/mini models are good enough for summarization. Users can override with a stronger model via settings.

**[Agent creates too many skills]** → Mitigated by system prompt guidance that constrains when to create (5+ tool calls, error recovery, user corrections). The agent can always `/skills` list and delete stale ones. No automatic creation — the agent must explicitly decide.

**[Memory bloat in system prompt]** → Mitigated by char limits (2200 global, 1375 project). At ~4 chars/token, this is ~900 tokens — small relative to a 200k context window.

**[Compression loses critical context]** → Mitigated by keeping recent messages intact and making compression opt-in (never automatic). The nudge is advisory only.

## Open Questions

- Should the compress tool save the pre-compression messages to a "conversation archive" in the DB for later retrieval? Hermes does this (session lineage). Probably not for v1 — the JSONL file still has the full history.
- Should memory entries have TTL/expiry? Hermes doesn't do this. Probably not — explicit curation is better than silent expiry.
- Should `session_search` return results as a blob ticket (via iroh) for large result sets, or inline? Inline for v1 — results are short previews, not full transcripts.
