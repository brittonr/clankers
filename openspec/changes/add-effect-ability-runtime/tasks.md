## Phase 1: Effect model

- [x] [serial] Define effect classes, request/result envelopes, correlation IDs, redaction rules, and handler trait boundaries. [covers=effect-ability-runtime.effect-classes] ✅ 3m 48s (started: 2026-05-16T23:33:08Z → completed: 2026-05-16T23:36:56Z; evidence: `cargo test -p clankers-runtime effects`)
- [x] [depends:effect-model] Map existing tool catalog capability packs and dangerous side-effect classes onto effect classes. [covers=effect-ability-runtime.catalog-mapping] ✅ 2m 22s (started: 2026-05-16T23:37:23Z → completed: 2026-05-16T23:39:45Z; evidence: `cargo test -p clankers-runtime`)

## Phase 2: Handler execution

- [x] [depends:effect-model] Add host-owned handlers for allow, deny, simulate, and replay modes for an initial file/shell/network/secret/tool subset. [covers=effect-ability-runtime.handlers] ✅ 1m 23s (started: 2026-05-16T23:40:06Z → completed: 2026-05-16T23:41:29Z; evidence: `cargo test -p clankers-runtime effects`)
- [ ] [depends:handlers] Route selected existing tool dispatch paths through effect handlers while preserving user-visible tool names and receipts. [covers=effect-ability-runtime.tool-dispatch]
- [ ] [depends:handlers] Add fail-closed sentinels for absent/denied handlers before filesystem, process, socket, browser, provider, or secret side effects. [covers=effect-ability-runtime.fail-closed]

## Phase 3: Remote dependency sync

- [ ] [depends:effect-model] Extend subagent/remote daemon execution requests to declare required skills, prompts, tool schemas, manifests, and policies by artifact hash. [covers=effect-ability-runtime.remote-deps]
- [ ] [depends:remote-deps] Implement safe missing-artifact sync and unsupported/missing/secret dependency failures. [covers=effect-ability-runtime.remote-sync]
- [ ] [serial] Run effect handler matrix tests, remote dependency sync tests, and catalog parity subset. [covers=effect-ability-runtime.validation]
