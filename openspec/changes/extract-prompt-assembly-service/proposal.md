## Why

System prompt assembly is valuable outside the terminal app, but embedding hosts need to decide whether to use Clankers discovery, provide app-native context, suppress local filesystem reads, or combine both. Prompt construction should become a reusable service with explicit policy and safe metadata rather than hidden behavior inside app shells.

## What Changes

- Extract prompt assembly into a policy-driven service that accepts host context, optional Clankers project discovery, SOUL/personality options, skills, AGENTS/CLAUDE files, OpenSpec context, and bounded context references explicitly.
- Allow hosts to disable filesystem/project discovery entirely.
- Return assembled prompt plus safe provenance metadata.

## Scope

In scope: prompt assembly API, policy flags, host context injection, no-filesystem mode, provenance metadata, and parity with existing Clankers prompt assembly.

Out of scope: changing the content or precedence of existing Clankers prompt sections unless needed to encode the current behavior explicitly.

## Verification

Validate with pure assembly tests, no-filesystem embedding tests, existing prompt parity fixtures, and metadata redaction tests.
