# Design: Decouple Config, Prompt, and Skill Services

## Summary

Reusable runtime and agent code should receive already-resolved settings, prompt sources, and skills through interfaces. Desktop Clankers can still read dotdirs, theme files, keybindings, system prompts, and skill roots, but that behavior should be explicitly wired at the shell edge.

## Decisions

### Decision: config core is display-neutral

Settings schemas and path resolution should not require TUI `Theme`, `ratatui::Color`, keymap widgets, router daemons, or UCAN runtime types unless those are feature-gated or moved to desktop adapters.

### Decision: prompt assembly is a host service

Prompt assembly should accept host-provided system text, project context, skill snippets, and context references. Embedded defaults should be host-context-only and fail closed for filesystem discovery unless enabled.

### Decision: skills are resolved through service traits

Skill roots and skill loading should be desktop services. Generic SDK code should see resolved skill content or an unavailable/unsupported service, not global filesystem lookup.

## Verification Plan

- Add dependency/source rails for config core against TUI/ratatui/router/runtime leakage.
- Add prompt assembly fixtures for host-only, filesystem-disabled, explicit desktop-enabled, and missing skill service cases.
- Add desktop parity tests for existing settings/theme/keybinding/prompt behavior through adapters.
