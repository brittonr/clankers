# Conversation-Level Prompt Caching

## Problem

Clankers only caches the system prompt (~8K tokens). Pi caches the entire
conversation prefix, achieving ~96% cache hit rates. On a 13-turn
file-reading session, this produces a **4.2× cost gap**:

| Agent    | Total Cost | Cache Strategy                  |
|----------|------------|---------------------------------|
| clankers | $1.49      | System prompt only (8K cached)  |
| pi       | $0.35      | Full conversation (50K cached)  |

Anthropic cache reads cost $0.30/MTok vs $3.00/MTok for uncached input —
a 10× difference. Conversation messages are the largest and most stable
part of each request, so caching them produces the biggest savings.

## Root Cause

Two code paths to Anthropic, neither caches conversation messages:

- **Provider path** (`clankers-provider/src/anthropic/api.rs`): caches
  system prompt + last tool. No `cache_control` on messages.
- **Router path** (`clankers-router/src/backends/anthropic.rs`): caches
  system prompt only. No `cache_control` on messages or tools.

## How Anthropic Prompt Caching Works

- Place `cache_control: {"type": "ephemeral"}` breakpoints on content blocks
- Up to **4 breakpoints** per request
- Cache is keyed on the exact token prefix up to each breakpoint
- Minimum 1024 tokens for a cache hit (Sonnet/Haiku)
- Cache writes cost 25% more; cache reads cost 90% less
- Default TTL: 5 minutes (extendable to 1h with beta header)

Conversations are append-only: each turn's request is the previous
request + new tool result + new assistant response + new user message.
The prefix is identical across turns, so a breakpoint on the last user
message produces near-perfect cache hits.

## Interaction: Compaction vs Caching

`compact_stale_tool_results()` replaces old tool results with summaries
like `[read: 3 lines, 50 bytes]`. This changes the token prefix when a
result transitions from "recent" (kept intact) to "stale" (compacted),
invalidating the cache for everything after that point.

The math favors caching over compaction:
- Compaction saves ~23% of context size (measured in multiturn-bench)
- Caching saves ~90% of cost per cached token
- At 70K context: compaction saves ~$0.048/turn, caching saves ~$0.189/turn

When they conflict, **caching wins by ~4×**. The plan makes compaction
monotonic so both work together.

---

## Phase 1: Add Conversation Cache Breakpoints

**Goal:** Place `cache_control` on the last user message's last content
block in both code paths. Expected result: cache_read jumps from 8K to
~50K+, cost drops ~4×.

### Task 1.1 — Add `cache_control` field to `ApiContentBlock`

**File:** `crates/clankers-provider/src/anthropic/api.rs`

The `ApiContentBlock` enum uses `#[serde(tag = "type")]`. Add an optional
`cache_control` field to the `Text` and `ToolResult` variants:

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub(crate) enum ApiContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Vec<ApiContentBlock>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    // Image, ToolUse, Thinking — no cache_control needed
    // ...
}
```

Update all construction sites to pass `cache_control: None` by default.

### Task 1.2 — Tag last user message in provider path

**File:** `crates/clankers-provider/src/anthropic/api.rs`, in `build_api_request()`

After `convert_messages()` returns `Vec<ApiMessage>`, find the last
message with `role == "user"` and set `cache_control` on its last content
block:

```rust
let mut messages = convert_messages(&request.messages);

// Conversation caching: tag last user message for prefix caching
if !request.no_cache {
    if let Some(last_user) = messages.iter_mut().rev().find(|m| m.role == "user") {
        if let Some(last_block) = last_user.content.last_mut() {
            last_block.set_cache_control(CacheControl::ephemeral());
        }
    }
}
```

Add a `set_cache_control` method on `ApiContentBlock`:

```rust
impl ApiContentBlock {
    pub fn set_cache_control(&mut self, cc: CacheControl) {
        match self {
            Self::Text { cache_control, .. } => *cache_control = Some(cc),
            Self::ToolResult { cache_control, .. } => *cache_control = Some(cc),
            _ => {} // no-op for Image, ToolUse, Thinking
        }
    }
}
```

### Task 1.3 — Tag last user message in router path

**File:** `crates/clankers-router/src/backends/anthropic.rs`, in `build_request_body()`

After messages are set on the body, mutate the JSON:

```rust
// Conversation caching: tag last user message
if let Some(messages) = body["messages"].as_array_mut() {
    for msg in messages.iter_mut().rev() {
        if msg["role"] == "user" {
            if let Some(content) = msg["content"].as_array_mut() {
                if let Some(last_block) = content.last_mut() {
                    last_block["cache_control"] = json!({"type": "ephemeral"});
                }
            }
            break;
        }
    }
}
```

### Task 1.4 — Fix missing tool caching in router path

**File:** `crates/clankers-router/src/backends/anthropic.rs`

The provider path caches the last tool, but the router path doesn't.
After building the tools array, tag the last one:

```rust
if !request.tools.is_empty() {
    let mut tools: Vec<Value> = request.tools.iter().map(/* ... */).collect();
    if let Some(last) = tools.last_mut() {
        last["cache_control"] = json!({"type": "ephemeral"});
    }
    body["tools"] = json!(tools);
}
```

### Task 1.5 — Wire up `--no-cache` flag

**Files:** `src/cli.rs`, provider path, router path

The `--no-cache` flag exists but is dead code. Thread it through:

1. Add `no_cache: bool` to `CompletionRequest` (both provider and router versions)
2. Set it from CLI args when building the request
3. Skip all `cache_control` insertion when `no_cache == true`

### Task 1.6 — Ensure beta header everywhere

**File:** `crates/clankers-router/src/backends/anthropic.rs`

The `prompt-caching-2024-07-31` beta header is only set in the router's
API key path (line 248), not the OAuth path. Add it to OAuth too, or
check if the newer API versions include caching by default.

---

## Phase 2: Make Compaction Cache-Safe

**Goal:** Compaction and caching work together without invalidating
cache prefixes.

### Task 2.1 — Monotonic compaction

**File:** `crates/clankers-agent/src/context.rs`

Current behavior: `compact_stale_tool_results(messages, keep_recent=3)`
compacts everything except the last 3 tool results. When a new tool
result arrives, result N-3 transitions from intact → compacted, changing
the prefix.

Fix: mark tool results as "compacted" in a way that's sticky. Two options:

**Option A — Compact immediately, keep none intact:**
Set `keep_recent = 0`. Every tool result is compacted on the turn after
it's created. The prefix never changes because results are compacted on
first insertion and stay that way. Downside: the model loses raw tool
output context (can still see summary + refer to file cache).

**Option B — Compact on creation, cache the compacted form:**
When a tool result enters the conversation, immediately store both the
full result (for the current turn) AND the compacted summary (for future
turns). On the next turn, swap to the compacted form. The transition
happens exactly once and at a predictable point.

**Recommended: Option A** for simplicity. The file cache in `clankers-db`
already persists full file contents, so the model can re-read if needed.
The compacted summaries (`[read: 250 lines, 12KB]`) give enough context
for the model to decide whether to re-read.

### Task 2.2 — Alternative: disable compaction when caching is active

If Option A hurts quality, just skip compaction entirely when prompt
caching is enabled. The cost savings from caching (90% reduction)
dwarf compaction savings (23% context reduction). Context growth
becomes linear but cheap.

Guard in `build_context()`:
```rust
let messages = if use_prompt_caching {
    messages.to_vec() // skip compaction
} else {
    compact_stale_tool_results(messages, 3)
};
```

---

## Phase 3: Verify

### Task 3.1 — Run multiturn-bench

```bash
./bench/multiturn-bench.sh
```

Expected results:
- clankers cache_read should grow from ~8K constant → ~50K+ growing
- Cost should drop from ~$1.49 → ~$0.35 (matching pi)
- Growth deceleration should still show if compaction is active

### Task 3.2 — Run token-bench.sh (regression check)

```bash
./bench/token-bench.sh
```

Single-prompt sessions should still work. Cache_write costs may increase
slightly (25% premium on first turn), but cache_read savings on
subsequent turns within a prompt should offset this.

### Task 3.3 — Manual multi-turn session

Run an interactive session with 10+ turns of file reads and verify:
- `--stats` shows cache_read growing each turn
- No unexpected cache misses (watch for compaction-induced invalidation)
- Quality is preserved (model can still reference previous file contents)

---

## Implementation Order

1. **Task 1.1 + 1.2** — Provider path conversation caching (biggest impact)
2. **Task 1.3 + 1.4** — Router path parity
3. **Task 3.1** — Verify with benchmark
4. **Task 2.1 or 2.2** — Fix compaction/caching interaction if cache
   misses are observed
5. **Task 1.5 + 1.6** — Polish (--no-cache, beta header)
6. **Task 3.2 + 3.3** — Full regression + manual verification

Estimated LOC: ~80 lines changed across 3 files. No new dependencies.
