# Group B: Remaining Infrastructure Extractions — Spec

## Purpose

Contracts for the three remaining infrastructure crates with zero internal
dependencies and moderate reverse dependency counts. These extractions are more
sensitive than the leaves because multiple workspace crates depend on them.

## Requirements

### protocol Extraction

The `clankers-protocol` crate MUST be extracted to `clanker-protocol`. The
framing layer (4-byte length prefix + JSON) and all command/event/control/types
modules MUST move together.

GIVEN `crates/clankers-protocol/` with modules `command`, `control`, `event`,
      `frame`, and `types`
WHEN extracted to the `clanker-protocol` repo
THEN `read_frame` and `write_frame` continue to use the existing tokio async
    framing implementation
AND `DaemonEvent`, `SessionCommand`, `ControlRequest`, and `ControlResponse`
    serialize/deserialize identically to the pre-extraction wire format
AND the root crate and `clankers-controller` compile via the migration wrapper

### protocol Wire Compatibility Verification

Wire-format compatibility MUST be proven with durable fixtures or equivalent
serde-framing tests, not inferred from a generic workspace build.

GIVEN the extracted `clanker-protocol` crate
WHEN protocol verification runs
THEN golden fixtures (or an equivalent checked-in fixture suite) cover
    `DaemonEvent`, `SessionCommand`, `ControlRequest`, and `ControlResponse`
AND the framing layer round-trips those fixtures without changing bytes or
    semantic meaning

### db Extraction

The `clankers-db` crate MUST be extracted to `clanker-db`. All table modules
must move together: audit, memory, sessions, history, usage, file_cache,
tool_results, and registry.

GIVEN `crates/clankers-db/` with redb and the existing schema/error modules
WHEN extracted to the `clanker-db` repo
THEN all table definitions and their read/write methods still work
AND the schema module continues to define the full redb table set
AND the error module preserves typed errors
AND the root crate and `clankers-agent` compile via the migration wrapper

### hooks Extraction

The `clankers-hooks` crate MUST be extracted to `clanker-hooks`. The dispatch
pipeline, hook points, verdicts, and script execution MUST all move.

GIVEN `crates/clankers-hooks/` with modules `config`, `dispatcher`, `git`,
      `payload`, `point`, `script`, and `verdict`
WHEN extracted to the `clanker-hooks` repo
THEN `HookPipeline`, `HookHandler`, `HookVerdict`, `HookPoint`, `HookPayload`,
    `HookConfig`, and `GitHooks` are public
AND the async `HookHandler` trait continues to work with tokio
AND script hook execution continues to work
AND `HookPoint` gains `Custom(String)` for non-clankers extensibility
AND the root crate, `clankers-agent`, `clankers-config`,
    `clankers-controller`, and `clankers-plugin` compile via the migration
    wrapper

### hooks Custom Variant Behavior

The new `HookPoint::Custom(String)` variant MUST round-trip cleanly and MUST
not break existing concrete hook points.

GIVEN a custom hook point name such as `"post_archive"`
WHEN it is serialized and deserialized through the extracted crate
THEN the result is `HookPoint::Custom("post_archive".into())`
AND existing concrete variants still round-trip unchanged

GIVEN a `HookPayload` targeting `HookPoint::Custom("post_archive".into())`
WHEN the dispatcher evaluates configured hooks
THEN custom hook handlers can match that hook point without affecting the
    behavior of existing built-in hook points
