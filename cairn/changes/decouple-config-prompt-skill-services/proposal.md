# Proposal: Decouple Config, Prompt, and Skill Services

## Problem

Configuration, prompt assembly, and skill loading still pull product concerns into reusable-looking crates. `clankers-config` depends on TUI/theme types and router/model/UCAN crates; prompt and skill assembly live in shell crates; embedded runtime defaults must avoid dotdirs and global service lookup. This blocks clean SDK composition for hosts that want their own settings, prompt, and skill sources.

## Proposed Change

Split neutral configuration DTOs and host service traits from desktop/TUI adapters. Prompt assembly and skill resolution should become injectable services with safe defaults and explicit desktop adapters; TUI theme/keymap conversion should live at the display edge.

## Impact

- **Files**: `crates/clankers-config`, `crates/clankers-prompts`, `crates/clankers-skills`, `crates/clankers-runtime/src/prompt.rs`, `src/runtime_services.rs`, TUI theme/keymap adapters.
- **Testing**: config dependency rail, prompt assembly fixtures, skill service absent/default behavior, desktop parity tests.
