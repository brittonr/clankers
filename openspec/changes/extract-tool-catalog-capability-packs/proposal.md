## Why

Embedding applications need precise control over which Clankers tools exist. Today tool construction is tied to Clankers mode/common wiring and mixes core tools, optional specialty tools, plugins, MCP, gateway policy, and disabled-tool filtering. Hosts need a reusable tool catalog builder with capability packs and app-native tool injection.

## What Changes

- Extract a reusable tool registry/catalog builder from mode-specific wiring.
- Define capability packs such as text-only, filesystem-readonly, filesystem-mutate, shell/process, git, web, browser, orchestration, plugins, MCP, Matrix, and gateway.
- Let host applications inject custom tools and policy before publication.

## Scope

In scope: builder API, capability pack policy, disabled-tool filtering, custom tool injection, and parity with existing built-in tool publication.

Out of scope: rewriting individual tools, changing tool schemas, or enabling dangerous tools by default for embedders.

## Verification

Validate with catalog unit tests, publication parity tests against current Clankers defaults, negative tests proving dangerous packs are absent unless enabled, and host custom-tool execution tests.
