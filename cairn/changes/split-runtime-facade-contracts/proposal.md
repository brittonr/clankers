# Change: Split Runtime Facade Contracts

## Why

`clankers-runtime` is classified as a host-facade crate, but its public surface still mixes green reusable contracts, yellow host-injection APIs, executable Steel/runtime policy, and desktop adapter shells. That is too coupled for SDK users: a type that looks embeddable may carry provider/auth/plugin/process/prompt filesystem/session dependencies unless the classification is explicit and enforced.

## What Changes

- Split runtime public exports into green contracts, yellow host-injection surfaces, and desktop adapter shells with deterministic inventory receipts.
- Move serializable reusable DTOs to neutral owners or clearly named contract modules that do not require ambient services.
- Keep executable Steel evaluation, filesystem/config discovery, provider/auth/plugin/process/session implementations, and clocks behind yellow host-injected services or root desktop adapters.
- Update generated SDK docs so only green contracts are advertised as default embeddable APIs.

## Impact

- **Files**: `crates/clankers-runtime/src/{lib.rs,services.rs,steel_orchestration.rs,...}`, `crates/clankers-adapters`, `clankers-tool-host`, generated SDK/API docs, runtime facade checkers, and root runtime service construction.
- **Testing**: runtime public API inventory rail, fail-closed default service tests, Steel contract split fixtures, SDK docs/lego rails, `cargo check --tests`, Cairn gates, and diff checks.
