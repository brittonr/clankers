## 🔴 Major / High-Impact

- [x] **Web Search & Fetch** — Kagi (`src/tools/web.rs`: search + fetch with Kagi API, fallback raw HTTP)
- [x] **AI-Powered Commit Tool** — Agentic git analysis, hunk-level staging, split commits, automatic changelog generation, conventional commit validation (`src/tools/commit.rs`)
- [x] **Model Roles** — Route different tasks to different models (`default`, `smol`, `slow`, `plan`, `commit`, `review`) with `/role` command and `settings.json` persistence (`src/config/model_roles.rs`)
- [x] **TTSR (Time Traveling Streamed Rules)** — Zero-context rules that inject only when regex triggers match the model's output stream mid-generation (`src/agent/ttsr.rs`, config in `.clankers/ttsr.json`)

## 🟡 Medium / Nice-to-Have

- [x] **Interactive Code Review (`/review`)** — Structured findings with priority levels (P0-P3), verdict rendering (`src/tools/review.rs`, `/review` slash command)
- [x] **Context Compaction (mature)** — LLM-powered summarization with auto-compact thresholds, strategy selection, fallback to truncation (`src/agent/compaction.rs`)
- [x] **Multi-Provider Auth** — `clankers-router` crate with multi-provider auth store, OpenAI-compatible backend (OpenAI/OpenRouter/Groq/DeepSeek/local), auto-discovery from env vars, RouterProvider with model alias routing, credential manager with OAuth refresh + file locking (`crates/clankers-router/`, `src/provider/router.rs`)
- [x] **Plan Mode** — `/plan` toggle for architecture-first workflow before edits (`src/modes/plan.rs`, slash command, restricted tool set)
- [x] **Image Generation** — Via Gemini or OpenRouter (`src/tools/image_gen.rs`)
- [x] **Ask Tool** — Structured multi-choice/multi-select questions to the user (`src/tools/ask.rs`, supports TUI channel or non-interactive fallback)

## 🟢 Polish / UX

- [x] **Prompt History Search** — `Ctrl+R` search across sessions (`src/tui/components/history_search.rs`, JSONL-backed store)
- [x] **`@file` Auto-Read** — Type `@path` in prompt to inject file contents inline, with line ranges and directory listing (`src/util/at_file.rs`)
- [x] **Native Performance Modules** — ripgrep-powered grep, syntect highlighting, ANSI-aware text utils (`src/tools/grep.rs` uses `ignore`+`grep-regex`+`grep-searcher` in-process; `src/util/syntax.rs` uses `syntect`; `src/util/ansi.rs` provides `strip_ansi`/`visible_width`/`truncate_visible`)
