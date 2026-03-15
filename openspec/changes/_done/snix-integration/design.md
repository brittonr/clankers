# snix-integration — Design

## Decisions

### Use nix-compat as a parsing library, not a runtime

**Choice:** Import `nix-compat` for its type definitions and parsers only.
Continue using the `nix` CLI for actual builds and store operations.
**Rationale:** nix-compat is a pure parsing library with no daemon
dependencies.  It gives us `StorePath`, `Derivation`, `FlakeRef`, and
`nixbase32` without any runtime coupling.  Build execution still goes
through the CLI where the daemon, sandboxing, and substituters already work.
**Alternatives considered:** Use snix-store to interact with the store
directly.  Rejected — we'd need to implement the full daemon protocol,
manage GC roots, and handle substitution.  The CLI already does this.

### In-process eval via snix-eval for read-only introspection only

**Choice:** Use snix-eval for `nix eval`-equivalent tasks: reading attribute
values, listing flake outputs, evaluating simple expressions.  Never use it
for build-triggering evaluation (IFD, fetchurl, etc.).
**Rationale:** snix-eval can evaluate pure Nix expressions fast and without
process overhead.  But it can't do import-from-derivation or network
fetches without snix-glue + snix-store + snix-build — which is the entire
snix stack.  Drawing the line at pure evaluation keeps the dependency surface
small and the failure modes simple.
**Alternatives considered:** Pull in snix-glue for full evaluation.
Rejected — drags in the entire build/store stack for marginal benefit.
The CLI handles impure evaluation fine.

### New crate clankers-nix, not inline in the tool

**Choice:** Create `crates/clankers-nix/` as a library crate wrapping snix
types.  The NixTool and NixEvalTool depend on it for parsing and evaluation.
**Rationale:** Keeps snix dependencies out of the main binary crate.
Other crates (clankers-agent for system prompt generation, clankers-db
for artifact tracking) can use the parsing functions without depending on
tool infrastructure.
**Alternatives considered:** Inline everything in `src/tools/nix/`.  Works
for phase 1 but gets messy when multiple consumers need the parsing.

### StorePath wrapper for agent-visible metadata

**Choice:** Wrap `nix_compat::store_path::StorePath` in a clankers-specific
`NixPath` type that adds agent-friendly fields: human name, output hash
algorithm, whether it's a derivation output.
**Rationale:** Raw `StorePath` is a parsing type.  The agent needs context:
"this is the `hello-2.12.1` package, built with sha256, it's the `out`
output."  The wrapper extracts that from the path + optional `.drv` lookup.
**Alternatives considered:** Pass raw `StorePath` through.  Too low-level
for agent consumption — the agent would need to understand nixbase32 and
store path conventions.

### Flake ref validation before CLI dispatch

**Choice:** Parse and validate flake references with `nix_compat::flakeref`
before spawning the nix CLI.  Return a typed error if the reference is
malformed.
**Rationale:** Today the agent passes arbitrary strings to `nix build .#foo`
and gets a cryptic error if the reference is wrong.  Validating first
gives the agent an immediate, actionable error ("unknown flake output 'foo',
available outputs are: ...") without a process spawn roundtrip.
**Alternatives considered:** Let nix produce the error.  Slower feedback
loop, error messages are harder for the agent to parse.

### Refscan as post-processor, not inline in every tool

**Choice:** Run refscan on tool output text after execution, not embedded
in each tool's output path.  A utility function in clankers-nix takes any
string, scans for `/nix/store/` references, and returns annotated metadata.
**Rationale:** Store path references appear in bash output, nix build
output, file contents, error messages — everywhere.  A single post-
processing pass catches them all without modifying each tool.
**Alternatives considered:** Modify each tool to detect store paths.
Duplicates logic across 20+ tools.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Agent Turn Loop                         │
│                                                             │
│  ┌─────────────┐   ┌──────────────┐   ┌─────────────────┐  │
│  │   NixTool   │   │  NixEvalTool │   │  Other tools    │  │
│  │  (build,    │   │  (in-process │   │  (bash, read,   │  │
│  │   run, etc) │   │   eval)      │   │   etc.)         │  │
│  └──────┬──────┘   └──────┬───────┘   └──────┬──────────┘  │
│         │                 │                   │             │
│         │     ┌───────────▼────────────┐      │             │
│         │     │    clankers-nix        │      │             │
│         │     │                        │      │             │
│         │     │  ┌──────────────────┐  │      │             │
│         ├─────┤  │  parse module    │  ├──────┘             │
│         │     │  │  - store_path()  │  │                    │
│         │     │  │  - derivation()  │  │  ◄── refscan       │
│         │     │  │  - flake_ref()   │  │      post-process  │
│         │     │  │  - refscan()     │  │                    │
│         │     │  └──────────────────┘  │                    │
│         │     │                        │                    │
│         │     │  ┌──────────────────┐  │                    │
│         │     │  │  eval module     │  │                    │
│         │     │  │  - evaluate()    │  │                    │
│         │     │  │  - introspect()  │  │                    │
│         │     │  └──────────────────┘  │                    │
│         │     └────────────────────────┘                    │
│         │                                                   │
│         ▼                                                   │
│  ┌──────────────┐                                           │
│  │  nix CLI     │  (builds, impure eval, store ops)         │
│  └──────────────┘                                           │
└─────────────────────────────────────────────────────────────┘
```

### Dependency graph

```
clankers-nix
  ├── nix-compat        (store paths, derivations, flakeref, nixbase32)
  ├── snix-eval         (in-process evaluation, phase 2)
  ├── snix-serde        (Nix→Rust deserialization, phase 2)
  └── snix-castore      (refscan only, phase 3)

src/tools/nix/
  ├── clankers-nix      (parsing, eval, refscan)
  └── (existing)        (CLI spawning, streaming, progress)
```

## Data Flow

### Enhanced NixTool output (phase 1)

```
Agent calls NixTool { subcommand: "build", args: [".#hello"] }
  │
  ├─ Parse flake ref ".#hello" via nix_compat::flakeref
  │   └─ Malformed? → return typed error immediately, no spawn
  │
  ├─ Spawn `nix build .#hello --log-format internal-json`
  │   └─ Stream progress as today
  │
  ├─ Parse build output paths via nix_compat::store_path
  │   ├─ /nix/store/abc123-hello-2.12.1
  │   │   → NixPath { name: "hello-2.12.1", hash: "abc123", drv: None }
  │   └─ Optionally read .drv to get inputs, outputs, builder
  │
  └─ Return structured result:
      {
        "exit_code": 0,
        "outputs": [
          { "path": "/nix/store/abc123-hello-2.12.1",
            "name": "hello-2.12.1",
            "store_hash": "abc123" }
        ],
        "build_log": "...(truncated)...",
        "messages": [...]
      }
```

### NixEvalTool (phase 2)

```
Agent calls NixEvalTool { expr: "builtins.attrNames (builtins.getFlake \".\").outputs" }
  │
  ├─ snix_eval::EvaluationBuilder::new_pure()
  │   .build_result(&expr, &cwd)
  │
  ├─ Value → serde → JSON
  │
  └─ Return: ["packages", "devShells", "checks", "nixosConfigurations"]

Agent calls NixEvalTool { expr: "with import ./. {}; lib.version" }
  │
  ├─ Pure eval attempt via snix-eval
  │   └─ If impure builtins needed → fall back to `nix eval --json`
  │
  └─ Return: "24.05"
```

### Refscan post-processing (phase 3)

```
Agent calls BashTool { command: "cat result/bin/hello" }
  │
  ├─ Execute command, get output
  │
  ├─ clankers_nix::refscan::scan_store_refs(output)
  │   └─ Found: ["/nix/store/xyz-glibc-2.38", "/nix/store/abc-hello-2.12.1"]
  │
  ├─ For each ref, parse via store_path
  │   └─ Annotate: "References nix packages: glibc-2.38, hello-2.12.1"
  │
  └─ Append annotation to tool result (agent sees dependencies)
```
