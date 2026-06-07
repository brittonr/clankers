# Design: Extract Prompt and Skill Service Contracts

## Boundary

The neutral owner exposes typed prompt source requests, skill lookup requests, redacted diagnostics, and deterministic prompt assembly inputs. It must not read project/global config, inspect the filesystem, parse frontmatter from disk, or depend on desktop path conventions.

## Host injection

`clankers-runtime` may reexport or depend on the neutral contracts, but default runtime constructors must return typed unavailable errors for prompt/skill behavior until the host injects services. Desktop adapters own `.clankers`, `.pi`, skill directories, and config fallback behavior.

## Documentation

Embedded SDK docs should show product-owned prompt/skill services as explicit inputs, not ambient Clankers defaults.
