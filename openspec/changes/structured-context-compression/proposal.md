## Why

Clankers' current context compaction is truncation-only: it drops middle messages when the context window fills up. This discards tool results, decisions, and working state with no summary. Hermes has a structured compressor that summarizes tool outputs inline, generates iterative LLM summaries with Active Task / Resolved / Pending sections, and uses token-budget tail protection. The gap means clankers loses context quality rapidly in long sessions.

## What Changes

- Replace truncation-only compaction with a multi-pass compression pipeline
- Add tool-result summarization as a cheap pre-pass (no LLM needed — pattern-match tool name + args to generate one-line descriptions)
- Add LLM-powered structured summarization with a template that tracks resolved questions, active task, and remaining work
- Support iterative summary updates when compression fires multiple times in a session
- Protect tail messages by token budget (not fixed count) to preserve recent context quality

## Capabilities

### New Capabilities
- `structured-compaction`: Multi-pass context compression with tool-result pruning, LLM summarization, iterative updates, and token-budget tail protection.

### Modified Capabilities

## Impact

- `crates/clankers-agent/src/compaction.rs` — rewrite compaction pipeline
- `crates/clankers-agent/src/tool/` — add per-tool summarization functions for the pre-pass
- `crates/clankers-provider/` — auxiliary model call for summarization
- `crates/clankers-config/src/settings.rs` — new compaction config options (summary model, threshold, tail budget)
- Existing `CompactionStrategy::Truncation` remains as fallback when no auxiliary model is available
