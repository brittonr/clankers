Evidence-ID: v2-example-dependency-feature
Task-ID: V2
Artifact-Type: machine-check-log
Covers: embeddable-agent-engine.external-consumer-example.fake-adapters, embeddable-agent-engine.external-consumer-example.dependency-graph-clean, embeddable-agent-engine.external-consumer-example.public-api-no-runtime-handles, embeddable-agent-engine.sdk-feature-default-policy.validated, embeddable-agent-engine.adapter-recipes.positive-negative-paths, embeddable-agent-engine.adapter-only-modular-coupling.host-runner-traits, embeddable-agent-engine.adapter-only-modular-coupling.application-edge-composition
Created: 2026-04-25T23:49:36Z
Status: pass

# V2 Example/dependency/feature evidence

The example binary contains assertions for positive text/tool/retry paths and negative model/tool/retry/event/cancel/usage/transcript paths. Passing execution means those assertions all held.

## Positive: standalone example executes all adapter scenarios

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running `/home/brittonr/.cargo-target/debug/embedded-agent-sdk-example`
embedded-agent-sdk example passed
```

## Positive: dependency denylist and feature/default policy check

```text
ok: embedded SDK example dependency graph has 56 packages and excludes forbidden runtime crates
```

## Negative: missing standalone workspace marker fails

```text
cargo metadata for embedded SDK example failed with status Some(101)
stdout:

stderr:
error: current package believes it's in a workspace when it's not:
current:   /home/brittonr/git/clankers/.pi/worktrees/session-1777159833772-0vld/examples/embedded-agent-sdk/Cargo.toml
workspace: /home/brittonr/git/clankers/.pi/worktrees/session-1777159833772-0vld/Cargo.toml

this may be fixable by adding `examples/embedded-agent-sdk` to the `workspace.members` array of the manifest located at: /home/brittonr/git/clankers/.pi/worktrees/session-1777159833772-0vld/Cargo.toml
Alternatively, to keep it out of the workspace, add the package to the `workspace.exclude` array, or add an empty `[workspace]` table to the package's manifest.

missing [workspace] failed as expected
```

## Negative: stale feature policy docs fail

```text
docs/src/tutorials/embedded-agent-sdk.md missing feature policy phrase: `clankers-engine`: no optional features
stale feature policy failed as expected
```
