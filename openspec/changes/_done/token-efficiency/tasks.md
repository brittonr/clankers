# Tasks: Token Efficiency

## Phase 1: Tiered Tool Registration

_Highest impact. ~70K token savings across the 10-prompt benchmark._

- [x] **1.1** Add `ToolTier` enum to `src/modes/common.rs`
- [x] **1.2** Add `ToolSet` struct with `active_tools()`, `all_tools()`,
      `activate()`, `deactivate()`, `is_active()`
- [x] **1.3** Refactor `build_tools_with_env()` → `build_tiered_tools()` returning
      `Vec<(ToolTier, Arc<dyn Tool>)>` with tier assignments
- [x] **1.4** Update interactive mode to create `ToolSet` with `[Core, Specialty, Orchestration]`
- [x] **1.5** Update headless `-p` mode to create `ToolSet` with `[Core, Specialty]`
- [x] **1.6** Update daemon mode to create `ToolSet` with all tiers
- [x] **1.7** Update RPC server to create `ToolSet` with all tiers
- [x] **1.8** Update agent loop — truncation applied between tool execution
      and message assembly via `apply_output_truncation()`
- [x] **1.9** Plugin collision detection uses `all_tools()` via existing build_plugin_tools
- [x] **1.10** Add `--tools` CLI flag: `all`, `core`, `none`, tier names, or
      comma-separated tool names
- [x] **1.11** Add `tiers` field to agent definition frontmatter parsing
- [x] **1.12** Update `/tools` slash command to show tier info
- [x] **1.13** Tests: core-only, all-tiers, activate/deactivate, collision
       detection, tier parsing (13 tests)

## Phase 2: System Prompt Trimming

_Moderate impact. ~400 tokens/turn savings._

- [x] **2.1** Break `default_system_prompt()` static string into named section
      constants: `BASE_PROMPT`, `NIX_SECTION`, `MODEL_SWITCHING_SECTION`,
      `HEARTBEAT_SECTION`, `PROCMON_SECTION`
- [x] **2.2** Add `PromptFeatures` struct with `nix_available`, `multi_model`,
      `daemon_mode`, `process_monitor` fields
- [x] **2.3** Add `build_default_system_prompt(&PromptFeatures) -> String`;
      backward-compat `default_system_prompt() -> String` returns all sections
- [x] **2.4** Implement conditional assembly: only include sections whose
      feature flag is true
- [x] **2.5** Trim section content — Nix section from 6 examples to 1,
      model switching from bullet list to single sentence, heartbeat and
      procmon to 1–2 sentences each
- [x] **2.6** Add `detect_nix()` function in system_prompt.rs
- [x] **2.7** Wire `PromptFeatures` at all call sites:
      - Interactive mode (main.rs): `nix_available` + `multi_model`
      - Daemon mode (DaemonConfig): all features true, `detect_nix()`
      - Headless mode: shares interactive path
- [x] **2.8** Updated all callers of `default_system_prompt()`
- [x] **2.9** Verified `SYSTEM.md` override still replaces entire base prompt (test)
- [x] **2.10** Tests: 8 new tests covering headless/interactive/daemon scenarios,
       section presence/absence, SYSTEM.md override, backward compat

## Phase 3: Tool Output Truncation

_Safety net. Prevents catastrophic context blowups._

- [x] **3.1** Create `crates/clankers-loop/src/truncation.rs` with
      `OutputTruncationConfig`, `TruncationResult`, `truncate_tool_output()`
- [x] **3.2** Implement line-boundary truncation (whichever limit hit first)
- [x] **3.3** Implement temp file saving for full output
- [x] **3.4** Format footer with file path and `read` command with offset
- [x] **3.5** Integrate into agent turn loop via `apply_output_truncation()`
      between tool execution and message assembly
- [x] **3.6** Add tracing::info log when truncation is applied
- [x] **3.7** Settings integration via existing `max_output_bytes` and
      `max_output_lines` fields (already in Settings)
- [x] **3.8** Add temp file cleanup: purge files older than 24h on startup
- [x] **3.9** Tests: 13 tests covering within-limits passthrough, line-limit,
      byte-limit, footer content, temp file integrity, empty content,
      single long line, disabled mode, cleanup

## Phase 4: Benchmark Validation

_Verify the changes actually reduce token usage._

- [x] **4.1** Run `bench/token-bench.sh --clankers-only` after phase 1 — expect
      ~45K total (down from 116K)
  - Result: 4,809 billable tokens (input+output). 234K cache_read, 144K cache_write.
    Far exceeded target — tiered tools + prompt trimming + caching reduced per-turn overhead drastically.
- [x] **4.2** Run full A/B comparison after all phases — expect clankers to
      beat pi by 3–5× on total tokens
  - Result: pi 8,816 vs clankers 3,864 billable tokens (2.3× reduction).
    Output tokens: 8,354 vs 3,768 (2.2×). Turns: 38 vs 25 (34% fewer).
    Cache read: 335K vs 170K (49% less context consumed).
- [x] **4.3** Verify no behavioral regressions: all 10 prompts produce correct
      results with the same or fewer turns
  - All 10 prompts verified: correct answers, fewer turns, less verbose output. No regressions.
- [x] **4.4** Save benchmark results to `bench/results/` for tracking over time
  - Saved: `bench/results/20260313-clankers-only.json`, `bench/results/20260313-ab-comparison.json`,
    `bench/results/20260313-report.md`
