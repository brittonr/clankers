# SOUL/Personality Module Inventory

## Existing ownership candidates

- `crates/clankers-agent/src/system_prompt.rs` owns prompt resource discovery and prompt assembly. It already loads global/project context files, `AGENTS.md`/`CLAUDE.md`, `SYSTEM.md`, `APPEND_SYSTEM.md`, OpenSpec context, skills, and settings prefix/suffix into one system prompt.
- `crates/clankers-agent/src/lib.rs` owns the active `Agent` system prompt and exposes `system_prompt()` / `set_system_prompt(...)`, so runtime personality switching should eventually update the same shell-facing prompt state.
- `src/main.rs` owns CLI dispatch and agent-mode assembly of `PromptResources`, provider construction, and the final system prompt passed into prompt/TUI/daemon paths.
- `src/modes/interactive.rs`, `src/modes/inline.rs`, `src/modes/json.rs`, RPC modes, and daemon socket/QUIC bridges all receive the assembled `system_prompt`; first-pass wiring should compose before those entrypoints rather than forking each agent loop.
- `src/modes/agent_task.rs` already routes `AgentCommand::SetSystemPrompt` / `GetSystemPrompt`, which is the likely TUI/runtime mutation seam for later `/personality` switching.
- `src/cli.rs`, `src/commands/`, and `src/tools/` are the consistent surfaces for the recent productionization slices (`checkpoint`, `tool_gateway`, `voice_mode`) and should host the first-pass SOUL/personality status/validate adapter.
- `crates/clankers-config/src/paths.rs` and project `.clankers/` paths are the right place to document discovery locations; first pass can validate explicit files/presets without adding new global mutable config.

## First-pass ownership recommendation

Land a small `src/soul_personality.rs` policy module first. It should validate SOUL/personality inputs and produce safe metadata without rewriting the entire prompt assembly pipeline. Then wire CLI/tool adapters through the existing command and Specialty tool registry.

## Boundaries to preserve

- Do not persist raw SOUL.md content or prompt text in replay metadata.
- Do not let remote URLs, shell commands, or provider-specific persona fetches run in the first pass.
- Keep persona composition as an explicit local-file/preset validation result until prompt assembly wiring is covered by targeted tests.
