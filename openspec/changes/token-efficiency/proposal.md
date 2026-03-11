# Proposal: Token Efficiency

## Problem

A/B benchmarks (10 prompts, same model, same repo) show clankers uses 2.5×
fewer total tokens than pi (116K vs 293K). But clankers still wastes tokens
on tool definitions that are never called, and pi's one catastrophic win
(gitignore-aware grep) points to a broader design question: how should the
tool set scale with task complexity?

Clankers registers 25 tools on every API call. Only 6 were used across the
entire 10-prompt benchmark. The unused 19 tools cost ~7,000 tokens per API
call — roughly 70K wasted tokens across the benchmark run.

### Benchmark Data

| Metric | pi | clankers | Δ |
|--------|---:|--------:|---|
| Turns | 39 | 36 | +8% |
| Input tokens | 1,035 | 112,175 | -99% |
| Output tokens | 6,825 | 4,407 | +55% |
| Cache read | 176,789 | 262,314 | -33% |
| Cache write | 108,200 | 90,801 | +19% |
| Total tokens | 292,849 | 116,582 | +151% |

Pi's token accounting splits cached tokens from input; clankers rolls them
together. The total-tokens comparison is the meaningful one.

### Root Causes

1. **Tool definition bloat** — 25 tools registered, 6 used. The subagent tool
   alone costs ~435 tokens due to nested JSON schema with parallel/chained
   task arrays. Matrix tools (6) are never used in non-daemon mode.

2. **System prompt overhead** — 799 tokens vs pi's 559. Includes sections on
   Nix packages, model switching, HEARTBEAT.md, and process monitoring that
   are irrelevant to most prompts.

3. **No tool output capping** — grep/find/bash can return unbounded output.
   Pi's worst prompt (#3) hit 138K tokens because raw `grep -rl` dumped
   .git/ and target/ contents. Clankers avoids this via gitignore-aware grep,
   but a sufficiently large repo could still blow up.

4. **Context accumulation** — each turn re-sends the full conversation. Verbose
   tool output in early turns compounds into later turns. A 51K bash result
   at turn 1 is re-sent at turns 2, 3, and 4.

## Proposed Solution

Three changes, ordered by impact:

1. **Tiered tool registration** — Register core tools (7) by default. Load
   orchestration, matrix, and specialty tools only when the context requires
   them or the user requests them.

2. **System prompt trimming** — Move conditional sections (HEARTBEAT, procmon,
   model switching, Nix) behind feature flags or context detection. The base
   prompt drops from ~799 to ~400 tokens.

3. **Tool output truncation** — Cap all tool results at a configurable limit
   (default: 50KB / 2000 lines, matching pi's behavior). Tools that exceed
   the limit save full output to a temp file and return a truncated result
   with a path reference.

## Scope

Three independent specs, each shippable alone:

1. **Tiered tool registration** (highest impact, ~70K token savings per benchmark)
2. **System prompt trimming** (moderate impact, ~400 tokens/turn)
3. **Tool output truncation** (safety net, prevents catastrophic blowups)
