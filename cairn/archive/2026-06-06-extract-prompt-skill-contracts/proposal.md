# Change: Extract Prompt and Skill Service Contracts

## Why

Prompt assembly and skill lookup are reusable behaviors, but the runtime facade still carries yellow host-service shapes that are close to filesystem, config, and project-state concerns. Embedders need a small contract they can implement without inheriting Clankers desktop defaults or path discovery.

## What Changes

- Extract prompt-source, skill-source, redaction, and prompt-render service DTOs/traits into a neutral contract owner.
- Keep filesystem discovery, `.clankers`/`.pi` fallbacks, config parsing, and desktop skill loading in shell adapters.
- Update docs and rails so runtime defaults fail closed unless a host injects prompt/skill services.

## Impact

- **Files**: `crates/clankers-runtime/src/prompt.rs`, `crates/clankers-runtime/src/services.rs`, prompt/skill config code, examples, generated runtime facade inventory, and prompt-service rails.
- **Testing**: config/prompt/skill service fixtures, runtime fail-closed defaults, embedded SDK acceptance, and Cairn gates.
