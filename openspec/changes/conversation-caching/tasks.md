# Conversation Caching — Task Checklist

## Phase 1: Add Conversation Cache Breakpoints

- [x] **1.1** Add `cache_control: Option<CacheControl>` to `ApiContentBlock::Text` and `ApiContentBlock::ToolResult`
  - File: `crates/clankers-provider/src/anthropic/api.rs`
  - Add `set_cache_control()` method
  - Update all construction sites to pass `cache_control: None`

- [x] **1.2** Tag last user message in provider path
  - File: `crates/clankers-provider/src/anthropic/api.rs`, `build_api_request()`
  - Find last `role == "user"` message, set `cache_control` on its last content block

- [x] **1.3** Tag last user message in router path
  - File: `crates/clankers-router/src/backends/anthropic.rs`, `build_request_body()`
  - Mutate JSON to add `cache_control` to last user message's last content block

- [x] **1.4** Fix missing tool caching in router path
  - File: `crates/clankers-router/src/backends/anthropic.rs`
  - Add `cache_control` to last tool in the tools array

## Phase 2: Make Compaction Cache-Safe

- [ ] **2.1** Choose compaction strategy (Option A: compact immediately, Option B: disable when caching)
- [ ] **2.2** Implement chosen strategy in `build_context()` / `compact_stale_tool_results()`
  - File: `crates/clankers-agent/src/context.rs`

## Phase 3: Verify

- [x] **3.1** Run `./bench/multiturn-bench.sh` — confirm cache_read grows, cost drops ~4×
  - Result: cost $1.49 → $0.45 (70% reduction), cache_read 96K → 502K
- [x] **3.2** Run `./bench/token-bench.sh` — confirm no regression on single-prompt
  - Result: 14% cheaper on single-prompt bench too
- [ ] **3.3** Manual multi-turn session with `--stats`

## Phase 4: Polish

- [ ] **4.1** Wire up `--no-cache` flag (currently dead code)
- [ ] **4.2** Ensure `prompt-caching-2024-07-31` beta header in all paths
- [ ] **4.3** Consider `ttl: "1h"` support for long sessions
