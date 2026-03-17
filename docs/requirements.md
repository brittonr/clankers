# Formal Verification Requirements

## Purpose

Requirements for machine-checked invariants on clankers' core data
structures. Each requirement maps to a Verus spec fn (formal definition),
a proof fn (machine-checked evidence), and an impl annotation on the
runtime code.

## Graggle Merge

r[merge.dag.sentinels]
A graggle MUST always contain ROOT and END vertices. `Graggle::new()`
produces a graggle where both exist and ROOT has a forward edge to END.

r[merge.dag.reachability]
Every alive content vertex in a graggle MUST be reachable from ROOT via
forward edges, and MUST be able to reach END via forward edges.

r[merge.dag.acyclicity]
The forward edge relation of a graggle MUST be acyclic. No vertex can
reach itself by following forward edges.

r[merge.insert.preserves-dag]
`insert_vertex` with valid up_context and down_context on a well-formed
graggle MUST produce a well-formed graggle (sentinels present, reachability
maintained, acyclicity maintained).

r[merge.order-independence]
For a base graggle and any set of branch texts, the merged content MUST be
identical regardless of the order branches are supplied to `merge()`.

r[merge.from-text.linear]
`Graggle::from_text(s)` MUST produce a linear chain: ROOT → line₀ → line₁
→ ... → lineₙ → END, where each lineᵢ vertex contains the i-th segment
of `s` split by `split_inclusive('\n')`.

r[merge.delete.ghost]
`delete_vertex(id)` MUST set `alive = false` on the vertex without removing
it from the graph. The vertex's edges MUST remain unchanged.

## Actor Registry

r[actor.link.bidirectional]
`link(a, b)` MUST add b to the link set of a AND add a to the link set of b.
After `link(a, b)`, both `links[a].contains(b)` and `links[b].contains(a)`
hold.

r[actor.unlink.bidirectional]
`unlink(a, b)` MUST remove b from the link set of a AND remove a from the
link set of b. After `unlink(a, b)`, neither direction is present.

r[actor.exit.link-cleanup]
When a process exits, `on_process_exit(id)` MUST remove id from the link
sets of all processes that were linked to id.

r[actor.exit.monitor-cleanup]
When a process exits, `on_process_exit(id)` MUST remove id from the monitor
map (as watched) and notify all watchers.

r[actor.name.unique]
The name-to-id map MUST be injective: no two live processes share the same
name. `spawn` with a name that is already registered overwrites the mapping.

## Session Tree

r[session.walk.path-valid]
`walk_branch(leaf_id)` MUST return a sequence where for each consecutive
pair (entries[i], entries[i+1]), entries[i+1].parent_id == Some(entries[i].id).

r[session.walk.root-anchored]
The first entry returned by `walk_branch` MUST have parent_id == None
(it is a root message).

r[session.walk.terminates]
`walk_branch` MUST terminate in O(n) steps where n is the number of messages.
It MUST NOT loop even if the underlying data contains a cycle (defensive).

r[session.index.consistent]
After `SessionTree::build(entries)`, for every (id, idx) in the index map,
`entries[idx]` MUST be a `SessionEntry::Message` with `id` matching the
key.

## Protocol Framing

r[protocol.frame.roundtrip]
For any value `v` where `serde_json::to_vec(v)` succeeds and the result
is ≤ MAX_FRAME_SIZE bytes, `read_frame(write_frame(v))` MUST yield a value
equal to `v`.

r[protocol.frame.size-reject-write]
`write_frame` MUST return `FrameError::TooLarge` if the serialized payload
exceeds MAX_FRAME_SIZE, without writing any bytes.

r[protocol.frame.size-reject-read]
`read_frame` MUST return `FrameError::TooLarge` if the 4-byte length header
indicates a size > MAX_FRAME_SIZE, without allocating a buffer of that size.

r[protocol.frame.length-encoding]
Frames MUST use 4-byte big-endian length prefix. The length field encodes
the payload size only, not including the 4 length bytes themselves.

r[protocol.frame.max-fits-u32]
`MAX_FRAME_SIZE` MUST be ≤ `u32::MAX`. This guarantees the `data.len() as
u32` cast in `write_frame` cannot truncate after the size check passes.

## UCAN Authorization

r[ucan.auth.no-escalation]
During delegation, if a parent capability contains a child capability,
then any operation the child authorizes MUST also be authorized by the
parent. No delegated token can grant access the parent does not have.

r[ucan.auth.read-only-blocks-write]
A FileAccess capability with read_only=true MUST authorize FileRead
operations on matching paths and MUST NOT authorize FileWrite operations
regardless of path.

r[ucan.auth.wildcard-matches-all]
A capability pattern of "*" MUST match any value in its domain — tool
names, shell commands, or bot commands.

r[ucan.auth.pattern-set-containment]
For comma-separated patterns, pattern P1 contains pattern P2 if and only
if every item in P2's set is also present in P1's set. A wildcard "*"
contains any pattern. No non-wildcard pattern contains "*".

r[ucan.gate.tool-check]
The capability gate MUST reject tool calls where no ToolUse capability in
the token matches the requested tool name.

r[ucan.gate.file-read-check]
For file read tools (read, rg, grep, find, ls), the capability gate MUST
verify a matching FileAccess capability whose prefix covers the file path.

r[ucan.gate.file-write-check]
For file write tools (write, edit), the capability gate MUST verify a
matching FileAccess capability with read_only=false whose prefix covers
the file path.

## Protocol Serde Stability

r[protocol.serde.request-discriminant]
`DaemonRequest` MUST serialize as internally-tagged JSON with discriminant
key `"type"`. The discriminant values MUST be exactly `"Control"` and
`"Attach"`.

r[protocol.serde.attach-response-discriminant]
`AttachResponse` MUST serialize as internally-tagged JSON with discriminant
key `"type"`. The discriminant values MUST be exactly `"Ok"` and `"Error"`.

r[protocol.serde.command-externally-tagged]
`SessionCommand` MUST use serde's default externally-tagged representation.
Unit variants serialize as the bare string `"Abort"`. Struct variants
serialize as `{"VariantName": {fields}}`.

r[protocol.serde.event-externally-tagged]
`DaemonEvent` MUST use serde's default externally-tagged representation.
Unit variants serialize as bare strings; struct variants as
`{"VariantName": {fields}}`.

## Protocol Handshake

r[protocol.handshake.version-field]
`PROTOCOL_VERSION` MUST be > 0. A well-formed `Handshake` MUST have
`protocol_version > 0`.

## Plugin Permission Model

r[plugin.perm.all-grants-every]
`has_permission(perms, p)` MUST return `true` for every `Permission`
variant `p` when `perms` contains the string `"all"`.

r[plugin.perm.explicit-match]
`has_permission(perms, p)` MUST return `true` when `perms` contains a
string equal to `p.as_str()`.

r[plugin.perm.deny-without-grant]
`has_permission(perms, p)` MUST return `false` when `perms` contains
neither `"all"` nor a string equal to `p.as_str()`.

r[plugin.perm.no-cross-grant]
Each `Permission` variant's `as_str()` MUST return a distinct string. No
`as_str()` result may equal `"all"`. Granting one permission string MUST
NOT cause `has_permission` to return `true` for any other `Permission`
variant.

## Plugin Host Function Gating

r[plugin.host.fs-read-gated]
`HostFunctions::execute` MUST check `has_permission(perms, FsRead)` before
executing `"read_file"` or `"list_dir"`. If the check fails, it MUST
return a failure result without performing filesystem I/O.

r[plugin.host.fs-write-gated]
`HostFunctions::execute` MUST check `has_permission(perms, FsWrite)` before
executing `"write_file"`. If the check fails, it MUST return a failure
result without performing filesystem I/O.

r[plugin.host.ungated-functions]
`"log"`, `"get_config"`, and `"get_env"` MUST NOT require any permission.
They MUST dispatch regardless of the permission set.

r[plugin.host.unknown-rejects]
`HostFunctions::execute` MUST return a failure result for any function name
not in the recognized set.

## Plugin UI Filtering

r[plugin.filter.strips-without-ui]
`filter_ui_actions(perms, actions)` MUST return an empty `Vec` when
`actions` is non-empty and `has_permission(perms, Ui)` is false.

r[plugin.filter.passes-with-ui]
`filter_ui_actions(perms, actions)` MUST return `actions` unchanged when
`has_permission(perms, Ui)` is true.

r[plugin.filter.empty-passthrough]
`filter_ui_actions(perms, vec![])` MUST return an empty `Vec` regardless of
permissions.

## Plugin Event Dispatch

r[plugin.event.parse-matches-agree]
For any string `s`, if `PluginEvent::parse(s) == Some(e)` and `e` is not
`PluginInit`, then `e.matches_event_kind(s)` MUST return `true`.

r[plugin.event.parse-complete]
Every `PluginEvent` variant MUST be reachable via `PluginEvent::parse` for
some string `s`.

r[plugin.event.unknown-rejects]
`PluginEvent::parse(s)` MUST return `None` for any string `s` that is not
one of the recognized event kind names.
