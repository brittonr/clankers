# aspen-backend — Gaps

Where aspen doesn't do enough (or does the wrong thing) for clankers.

---

## GAP 1: No Real-Time Token Streaming Primitive ⛔ CRITICAL

**The problem.** Clankers' hot path is an in-process pipeline:

```
LLM SSE → mpsc::channel → ContentDelta → broadcast::Sender<AgentEvent> → TUI
```

Latency between stages is sub-microsecond (tokio channel send = pointer swap).
An LLM streams ~50 tokens/second, and the TUI renders at 60fps. Any added
latency > 16ms causes visible stutter.

Aspen has **no mechanism for streaming arbitrary ephemeral data** between nodes:

- **KV watch** — only emits on state changes, requires Raft commit + fsync (2-5ms per event minimum)
- **Gossip** — unordered, best-effort, 10-100ms
- **Hooks** — post-commit reactions, not real-time
- **Job progress** — poll-based (`job_status`), `job_watch` may not be fully implemented
- **No bidirectional streaming RPC** — client-server protocol is request-response

The openspec's `distributed-agents.md` shows `JobEvent::Progress(msg)` but
never specifies HOW token deltas get from the worker node back to the
requesting client in real-time. Writing each token to KV and reading via
watch would add 2-5ms per token — completely unacceptable.

**What's needed.** Either:
1. Aspen grows an ephemeral pub/sub channel (not KV-backed) for real-time streaming
2. Clankers maintains a direct QUIC stream to the executing worker, bypassing aspen for the hot path
3. Distributed agents are scoped to non-interactive subagent jobs where streaming isn't needed

---

## GAP 2: Tool Execution Locality ⛔ CRITICAL

**The problem.** Every clankers tool executes on the **local filesystem**:
`bash`, `read`, `write`, `edit`, `find`, `grep`, `ls` — all operate on local
files. The agent calls `tool.execute()` inline in the turn loop.

The openspec's distributed agents spec shows a `ClankerAgentWorker` that
runs on any cluster node. But when a user on Node A submits a prompt that
Node B picks up:

- Node B's `bash` tool runs on Node B's filesystem
- Node B's `read`/`write`/`edit` tools access Node B's files
- The user on Node A expected to work on **Node A's files**

The openspec has **zero discussion** of:
- Remote filesystem access (NFS, FUSE, aspen-fuse, file forwarding)
- Tool execution delegation (run tools on the originating node, agent elsewhere)
- Working directory synchronization
- Git repo state transfer between nodes

This is the most fundamental assumption conflict. The entire value of a
coding agent is reading your code, editing your files, running your tests.
Moving the agent to a different node breaks all of that.

---

## GAP 3: In-Memory Conversation vs. KV Round-Trips 🔴 HIGH

**The problem.** The Agent struct holds `messages: Vec<AgentMessage>` — the
full conversation history in memory. Every turn reads from this Vec to build
the LLM request (O(1) access).

The openspec stores turns as individual KV entries:
```
clankers:sessions:{id}:turns:{seq:08} → serialized turn
```

To build context for the next LLM call, the agent needs to:
1. `scan("clankers:sessions:{id}:turns:")` — prefix scan through Raft
2. Deserialize each turn from JSON
3. Assemble into `Vec<AgentMessage>`

The daemon creates a fresh Agent per prompt in `run_session_prompt()` and
seeds it with `agent.seed_messages(history)`. With aspen, every incoming
prompt requires full conversation reconstruction from KV. For a 50-turn
session, that's 50+ KV reads before the agent can even start.

**What's unspecified:**
- Local caching (read-through with aspen as backing store)
- Hybrid mode (in-memory primary, async replication to aspen)
- Blob-based conversation snapshots instead of per-turn KV entries

---

## GAP 4: Session Tree Model Mismatch 🟡 MEDIUM

**The problem.** Clankers sessions are **tree-structured**, not linear.
`SessionTree` (`src/session/tree.rs`) supports branching conversations:
`record_branch` creates fork points, `build_messages_for_branch` walks a
specific path. MessageEntry has `parent_id` forming a DAG.

The openspec models sessions as a flat sequence:
```
clankers:sessions:{id}:turns:{seq:08}
```

Zero-padded sequence numbers assume linear ordering. No concept of branching,
parent_id, or tree structure. `append_turn` uses CAS on turn_count — this
fundamentally assumes a linear append log.

**What gets lost:**
- Branch points (user rewinds to a fork, explores different paths)
- Tree walking (`load_turns(from_seq, limit)` can't express "walk leaf→root")
- Session resume after branching (current code tracks `active_leaf_id`)

The key schema needs something like:
```
clankers:sessions:{id}:msgs:{msg-id}       → { parent_id, content, ... }
clankers:sessions:{id}:branches:{branch-id} → { leaf_msg_id }
```
But graph traversal over KV prefix-scan is expensive.

---

## GAP 5: Distributed Lock Latency for Interactive Use 🟡 MEDIUM

**The problem.** The daemon serializes prompts per-session using
`Arc<Mutex<()>>` — nanosecond lock acquisition. The openspec replaces this
with aspen's `DistributedLock`:

```
Lock key: clankers:locks:session:{session-id}
TTL: 300s, retry: linear(10, 200ms)
```

Minimum 2-5ms acquisition (Raft round-trip) even uncontended. Under
contention across nodes, the linear retry at 200ms intervals means a
waiting prompt could block for up to 2 seconds.

A user sending rapid follow-up prompts hits this lock every time. With
local Mutex, the second prompt queues behind the first with ~0 overhead.

**What's unspecified:**
- Queuing vs. fail-fast semantics
- Lock holder notification on completion
- Session affinity (route same user → same node to avoid distributed locking)

---

## GAP 6: Blob Eventual Consistency 🟡 MEDIUM

**The problem.** Blob operations bypass Raft and are eventually consistent.
If an agent on Node A stores a tool result as a blob and writes a BlobRef
to KV, then the session is accessed from Node B:

1. KV read succeeds (linearizable) — Node B sees the turn with BlobRef
2. Blob fetch may **fail** — not replicated to Node B yet

The openspec acknowledges P2P fetch ("check local → miss → ask iroh") but
treats it as transparent. In practice, iroh-blobs discovery + transfer adds
latency and fails if the originating node is offline.

**What's unspecified:**
- Read-after-write consistency for the blob-ref-in-KV pattern
- Fallback when originating node is unreachable
- Eager replication policy for "hot" blobs in active sessions

---

## GAP 7: WorkStore Not Addressed 🟢 LOW-MEDIUM

**The problem.** Clankers has a `WorkStore` (`src/work/`) — a redb-backed
task graph with dependencies, priorities, statuses, and agent assignment.
The openspec completely ignores it. The `ClankerStorage` trait has no
work-item operations. The key schema has no `clankers:work:*` prefix.

In cluster mode, work items should be visible across nodes (agents claim
work, see dependencies). This is exactly what benefits from distributed state.

---

## Recommended Resolution

The most pragmatic architecture: **session affinity for interactive work,
distribution only for explicit subagent delegation.**

| Concern | Interactive (TUI/daemon) | Subagent delegation |
|---------|------------------------|-------------------|
| Agent execution | Local to accepting node | Distributed via aspen-jobs |
| Tool execution | Local filesystem | Local to worker node's filesystem |
| LLM streaming | In-process channels | Not needed (result-only) |
| Session locking | Local Mutex (same node) | Distributed lock |
| Conversation state | In-memory + async replicate to KV | Ephemeral (job payload) |

Aspen provides durable storage (sessions, config, usage), shared
identity/discovery, auth (UCAN), and cross-node subagent dispatch.
But the **interactive agent turn loop stays local** to the accepting
node with in-process streaming and local tool execution preserved.

This means:
- Gap 1 is avoided for interactive sessions (streaming stays in-process)
- Gap 2 is scoped to subagent jobs (which bring their own working directory)
- Gap 3 is mitigated with a read-through cache (in-memory primary, async KV replication)
- Gap 4 is solvable with a tree-aware KV schema
- Gap 5 is avoided for interactive sessions (local Mutex when session-affine)
- Gap 6 is acceptable for subagent results (eventual consistency is fine)
