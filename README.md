# clankers

**clankers** is a terminal-based coding agent written in Rust. It connects to LLM providers (Anthropic, OpenAI, and others) and gives the model direct access to your development environment through a rich set of built-in tools — including file reading and writing, shell command execution, code search with ripgrep, surgical text editing, and more. Whether you need to explore an unfamiliar codebase, refactor a module, debug a failing test, or scaffold an entire project from scratch, clankers operates right where you already work: your terminal.

## Features

clankers ships with a comprehensive toolkit designed for real-world software development. The built-in tools include `bash` for shell execution, `read` and `write` for file I/O, `edit` for precise find-and-replace modifications, `grep` and `find` for code search and file discovery, `ls` for directory listing, `subagent` for delegating tasks to ephemeral sub-instances, and `delegate` for persistent swarm workers. It also supports a WebAssembly-based plugin system via Extism, so you can extend clankers with custom tools written in any language that compiles to Wasm. Sessions are automatically persisted as JSONL, allowing you to resume previous conversations with `--continue` or `--resume <id>`, and the agent supports skills — reusable prompt snippets that teach it domain-specific knowledge.

## Getting Started

To build clankers from source, clone the repository and run `cargo build --release`. You'll need a Rust toolchain (edition 2024). Authentication is managed via `clankers auth login` for OAuth or `clankers auth set-key <provider>` for API keys; alternatively, set the `ANTHROPIC_API_KEY` environment variable. Once authenticated, simply run `clankers` to launch an interactive TUI session. For non-interactive use, pass a prompt directly with `clankers -p "your prompt here"`, pipe input via `--stdin`, or select an output format with `--mode json|markdown|plain`. clankers also integrates with [Zellij](https://zellij.dev/) for terminal multiplexing and supports remote pair-programming through `clankers share` and `clankers join`.

## Configuration & Agents

clankers uses a layered configuration system: global settings live in `~/.config/clankers/`, while project-level overrides go in `.clankers/` at your repository root. Run `clankers config paths` to see all resolved locations, or `clankers config edit` to open your settings file in `$EDITOR`. Custom agent definitions let you create specialized personas with their own system prompts, models, and tool restrictions — manage them with `clankers agent new`, `clankers agent list`, and `clankers agent show`. You can tune model parameters at the CLI level too, including `--model`, `--temperature`, `--top-p`, `--max-tokens`, and `--thinking` for extended chain-of-thought reasoning. Skills can be installed from URLs or local paths with `clankers skill install` and loaded per-session with `--skill <name>`.

## License

clankers is released under the [GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)](https://www.gnu.org/licenses/agpl-3.0.html). Contributions are welcome — feel free to open issues or submit pull requests.
