## Context

Clankers' `compaction.rs` supports two strategies: `Truncation` (drop middle messages) and `LlmSummary` (placeholder, not fully implemented). The `AutoCompactConfig` triggers at 80% context usage and preserves a fixed count of recent messages. Tool results — often the largest context consumers — are dropped whole with no summary.

Hermes' `ContextCompressor` is a multi-pass pipeline: (1) prune old tool results with per-tool-type one-line summaries, (2) protect the recent tail by token budget while retaining the existing leading context convention, (3) LLM-summarize middle turns with a structured template, (4) iteratively update the summary on subsequent compressions. For clankers, the structured summary contract is the narrower set used throughout this change: Active Task, Key Decisions Made, Files Modified, and Remaining Work.

## Goals / Non-Goals

**Goals:**
- Tool-result pruning pre-pass: replace old tool outputs with informative one-line summaries (no LLM needed)
- LLM-powered structured summarization with a template that tracks active task, key decisions, files modified, and remaining work
- Iterative summary updates: when compaction fires again, feed the previous summary to produce an updated one
- Token-budget tail protection instead of fixed message count
- Summary prefix that frames it as a handoff from a previous context window, not active instructions
- Configurable summary model

**Non-Goals:**
- Lossless compression (this is intentionally lossy)
- Replacing the session persistence layer (full history stays in Automerge)
- Compression across sessions (only within a single conversation)

## Decisions

**Three-phase pipeline:**
1. **Tool-result pruning** (cheap, synchronous): Walk messages, replace tool results older than the tail window with one-line summaries based on tool name and arguments. Pattern-match common tools: bash → show command + exit code + line count; read → show path + char count; write → show path + line count; grep → show pattern + match count. Generic fallback for unknown tools.
2. **Token-budget tail selection**: Calculate tokens for the most recent messages working backward until the tail budget is consumed. Default tail budget: 40% of context window. This adapts to context window size automatically.
3. **LLM summarization**: Send middle messages (between the retained leading context and protected tail) to an auxiliary model with a structured prompt. The summary template has: `## Active Task`, `## Key Decisions Made`, `## Files Modified`, `## Remaining Work`. On subsequent compressions, include the previous summary in the prompt so the model produces a cumulative update.

**Summary framing:** Prefix the summary with a handoff notice (similar to Hermes' `SUMMARY_PREFIX`) that tells the model to treat it as background reference, respond only to the latest user message, and not re-execute completed tasks from the summary.

**Fallback:** If no auxiliary model is available or the summarization call fails, fall back to truncation (existing behavior). Never block the conversation because compaction failed.

**Store summaries for recovery:** Write the compaction summary to `clankers-db/tool_results` so it can be recovered if the session is replayed or the summary needs debugging.

## Risks / Trade-offs

- **Summarization quality:** Cheap models may miss nuance. Mitigate with a well-structured prompt and specific sections. The summary doesn't need to be perfect — it's a safety net against total context loss.
- **Latency:** LLM summarization adds 2-5 seconds per compaction. Acceptable since compaction is infrequent (once per ~50k tokens of conversation). Run the summarization call concurrently with the user thinking about their next message.
- **Tool-specific summarizers:** Each tool type needs a summarization function. Start with the 6 most common (bash, read, write, grep, edit, subagent) and use a generic fallback for the rest. Expand coverage over time.
- **Summary drift:** Iterative updates could accumulate inaccuracies. Mitigate by including raw message excerpts (not just prior summary) in subsequent summarization prompts.
