# Clankers

A terminal coding agent in Rust. Inspired by [pi](https://pi.dev), built to be hacked on.

## What is it?

Clankers is an interactive coding agent that lives in your terminal. It talks to LLM providers (Anthropic, OpenAI, Google, and others), executes tools (file I/O, shell commands, code search), and persists conversations as branching session trees.

It runs as a daemon with an actor system managing concurrent sessions, or standalone in a single terminal. Clients attach over Unix sockets locally or iroh QUIC remotely.

## Key features

- **Multi-provider routing** — automatic failover across providers, complexity-based model selection, budget enforcement
- **Daemon mode** — background agent sessions with attach/detach, like tmux for AI
- **Conversation branching** — fork, compare, merge, cherry-pick across conversation branches
- **WASM plugins** — extend the tool set with WebAssembly modules
- **P2P networking** — iroh QUIC for remote daemon access, session sharing, and agent-to-agent RPC
- **Matrix bridge** — multi-agent coordination over encrypted Matrix channels
- **Worktree isolation** — parallel sessions in separate git worktrees with LLM-powered merge

## Quick start

```bash
cargo build --release
export ANTHROPIC_API_KEY=sk-...
clankers
```

Or with the daemon:

```bash
clankers daemon start -d
clankers attach --new
```
