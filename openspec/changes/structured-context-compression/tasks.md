## 1. Tool-result pruning pre-pass

- [ ] 1.1 Create `crates/clankers-agent/src/compaction/tool_summaries.rs` with `summarize_tool_result(tool_name, args, content) -> String`
- [ ] 1.2 Implement summarizers for core tools: bash (command + exit code + line count), read (path + offset + char count), write (path + line count), grep/rg (pattern + match count), edit (path + edit count), subagent (goal + result size)
- [ ] 1.3 Implement generic fallback summarizer: `[tool_name] arg1=val1 (N chars result)`
- [ ] 1.4 Implement `prune_tool_results(messages, tail_start_idx) -> Vec<AgentMessage>` that replaces tool results before tail_start_idx with one-line summaries

## 2. Token-budget tail protection

- [ ] 2.1 Add `compaction.tail_budget_fraction` config option (default 0.40) in `crates/clankers-config/src/settings.rs`
- [ ] 2.2 Implement `select_tail_by_budget(messages, budget_tokens) -> usize` returning the index where the tail starts
- [ ] 2.3 Replace fixed `keep_recent` count with budget-based tail selection in `AutoCompactConfig`

## 3. LLM-powered structured summarization

- [ ] 3.1 Add `compaction.summary_model` config option (default: haiku) in settings
- [ ] 3.2 Define the structured summary prompt template with sections: Active Task, Key Decisions Made, Files Modified, Remaining Work
- [ ] 3.3 Define the `SUMMARY_PREFIX` handoff notice text
- [ ] 3.4 Implement `summarize_middle(messages, previous_summary, provider) -> String` that calls the auxiliary model
- [ ] 3.5 Wire into `compact()` function: prune tool results → select tail → summarize middle → assemble result

## 4. Iterative summary updates

- [ ] 4.1 Store the compaction summary text in `CompactionResult` and persist in session state
- [ ] 4.2 On subsequent compaction, pass previous summary into `summarize_middle` prompt
- [ ] 4.3 Save compaction summaries to `clankers-db/tool_results` for recovery

## 5. Fallback and integration

- [ ] 5.1 If auxiliary model call fails or times out, fall back to truncation-only (existing behavior)
- [ ] 5.2 Run summarization call with a 30-second timeout
- [ ] 5.3 Update `CompactionStrategy` enum: add `Structured` variant alongside existing `Truncation` and `LlmSummary`
- [ ] 5.4 Make `Structured` the default strategy when an auxiliary model is configured

## 6. Tests

- [ ] 6.1 Unit test: tool-result summarizers produce correct one-liners for each tool type
- [ ] 6.2 Unit test: tail budget selection preserves correct number of messages for various context sizes
- [ ] 6.3 Unit test: handoff prefix is present in compacted output
- [ ] 6.4 Integration test: full compaction pipeline with mock provider
