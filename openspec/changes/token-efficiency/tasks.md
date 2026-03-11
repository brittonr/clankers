# Tasks: Token Efficiency

## Phase 1: Tiered Tool Registration

_Highest impact. ~70K token savings across the 10-prompt benchmark._

- [ ] **1.1** Add `ToolTier` enum to `src/modes/common.rs`
- [ ] **1.2** Add `ToolSet` struct with `active_tools()`, `all_tools()`,
      `activate()`, `deactivate()`, `is_active()`
- [ ] **1.3** Refactor `build_tools_with_env()` → `build_all_tools()` returning
      `Vec<(ToolTier, Arc<dyn Tool>)>` with tier assignments
- [ ] **1.4** Update interactive mode to create `ToolSet` with `[Core, Specialty]`
- [ ] **1.5** Update headless `-p` mode to create `ToolSet` with `[Core]`
- [ ] **1.6** Update daemon mode to create `ToolSet` with all tiers
- [ ] **1.7** Update RPC server to create `ToolSet` with all tiers
- [ ] **1.8** Update agent loop to call `tool_set.active_tools()` instead of
      using flat tool vec
- [ ] **1.9** Update plugin collision detection to use `tool_set.all_tools()`
- [ ] **1.10** Add `--tools` CLI flag: `all`, `core`, `none`, `auto` (default)
- [ ] **1.11** Add `tiers` field to agent definition YAML parsing
- [ ] **1.12** Update `/tools` slash command to show tier info
- [ ] **1.13** Tests: core-only, all-tiers, activate/deactivate, collision
       detection, mode defaults

## Phase 2: System Prompt Trimming

_Moderate impact. ~400 tokens/turn savings._

- [ ] **2.1** Break `default_system_prompt()` static string into named section
      constants: `BASE_PROMPT`, `NIX_SECTION`, `MODEL_SWITCHING_SECTION`,
      `HEARTBEAT_SECTION`, `PROCMON_SECTION`
- [ ] **2.2** Add `PromptFeatures` struct with `nix_available`, `multi_model`,
      `daemon_mode`, `process_monitor` fields
- [ ] **2.3** Change `default_system_prompt()` signature from `() -> &'static str`
      to `(&PromptFeatures) -> String`
- [ ] **2.4** Implement conditional assembly: only include sections whose
      feature flag is true
- [ ] **2.5** Trim section content — Nix section from 6 examples to 1,
      model switching from bullet list to single sentence, heartbeat and
      procmon to 1–2 sentences each
- [ ] **2.6** Add `detect_nix()` function, cache result in startup context
- [ ] **2.7** Wire `PromptFeatures` at all call sites:
      - Interactive mode: `nix_available` + `multi_model`
      - Daemon mode: all features true
      - Headless mode: `nix_available` only
- [ ] **2.8** Update all callers of `default_system_prompt()` to pass features
- [ ] **2.9** Verify `SYSTEM.md` override still replaces entire base prompt
- [ ] **2.10** Tests: each scenario (headless/interactive/daemon × nix/no-nix),
       SYSTEM.md override, section presence/absence

## Phase 3: Tool Output Truncation

_Safety net. Prevents catastrophic context blowups._

- [ ] **3.1** Create `crates/clankers-loop/src/truncation.rs` with
      `TruncationConfig`, `TruncationResult`, `truncate_tool_output()`
- [ ] **3.2** Implement line-boundary truncation (whichever limit hit first)
- [ ] **3.3** Implement temp file saving for full output
- [ ] **3.4** Format footer with file path and `read` command with offset
- [ ] **3.5** Integrate into agent loop: apply truncation between tool execution
      and message assembly
- [ ] **3.6** Add tracing::info log when truncation is applied
- [ ] **3.7** Add settings integration: `[tools] max_output_bytes`,
      `max_output_lines`, `truncate_output`
- [ ] **3.8** Add temp file cleanup: purge files older than 24h on startup
- [ ] **3.9** Tests: within-limits passthrough, line-limit, byte-limit,
      footer content, temp file integrity, empty content, single long line

## Phase 4: Benchmark Validation

_Verify the changes actually reduce token usage._

- [ ] **4.1** Run `bench/token-bench.sh --clankers-only` after phase 1 — expect
      ~45K total (down from 116K)
- [ ] **4.2** Run full A/B comparison after all phases — expect clankers to
      beat pi by 3–5× on total tokens
- [ ] **4.3** Verify no behavioral regressions: all 10 prompts produce correct
      results with the same or fewer turns
- [ ] **4.4** Save benchmark results to `bench/results/` for tracking over time
