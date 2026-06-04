## Why

An embedding application often owns its own config, credential storage, database, cache, session store, project context roots, and plugin directories. Clankers should not force embedded hosts through global dotdirs, JSONL files, or CLI auth stores unless the host opts into the default desktop/terminal behavior.

## What Changes

- Add injectable runtime services for settings, auth/provider credentials, session persistence, cache/database, project context resolution, skill roots, plugin roots, and checkpoint backend selection.
- Provide in-memory/noop implementations for minimal embedding tests.
- Keep existing CLI/TUI/daemon filesystem defaults as adapters over the same interfaces.

## Scope

In scope: trait/config boundary, default adapters, in-memory/noop implementations, and tests proving no ambient global path access in minimal embedded runtime construction.

Out of scope: removing existing Clankers path conventions or rewriting provider auth internals beyond dependency injection seams.

## Verification

Validate with minimal embedded runtime tests under a temp home, store-injection tests, and legacy CLI path parity tests.
