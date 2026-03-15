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

- [x] **2.1** Choose compaction strategy (Option A: compact immediately, Option B: disable when caching)
  - Chose Option B: skip compaction when caching is active. Caching saves ~90% on reads vs compaction's ~23% context reduction. LLM auto-compaction handles overflow.
- [x] **2.2** Implement chosen strategy in `build_context()` / `compact_stale_tool_results()`
  - File: `crates/clankers-agent/src/context.rs`
  - `build_context()` accepts `compact: bool`; `prepare_turn_context()` passes `self.settings.no_cache` (compact only when caching disabled)

## Phase 3: Verify

- [x] **3.1** Run `./bench/multiturn-bench.sh` — confirm cache_read grows, cost drops ~4×
  - Result: cost $1.49 → $0.45 (70% reduction), cache_read 96K → 502K
- [x] **3.2** Run `./bench/token-bench.sh` — confirm no regression on single-prompt
  - Result: 14% cheaper on single-prompt bench too
- [x] **3.3** Manual multi-turn session with `--stats`
  - Added cache_read/cache_creation stats to print mode `--stats` output

## Phase 4: Polish

- [x] **4.1** Wire up `--no-cache` flag (currently dead code)
  - Added `no_cache: bool` to Settings, both CompletionRequest types, TurnConfig
  - Threaded from CLI → Settings → Agent → TurnConfig → CompletionRequest → provider/router
  - All cache_control insertion guarded behind `!no_cache`
- [x] **4.2** Ensure `prompt-caching-2024-07-31` beta header in all paths
  - Added to provider path: both OAuth and API key branches
  - Added to router path: OAuth branch (API key already had it)
- [x] **4.3** `ttl` support for long sessions
  - Added `cache_ttl` setting (None = 5m default, "1h" = 1-hour at 2× base input cost)
  - `--cache-ttl 1h` CLI flag threads through Settings → TurnConfig → CompletionRequest → API
  - Provider path: `CacheControl::with_ttl()` serializes optional `ttl` field
  - Router path: `cache_control_json()` helper builds JSON with optional `ttl`
  - Also configurable in settings.json: `"cache_ttl": "1h"`
