# In-Process Evaluation

## Purpose

Evaluate Nix expressions in-process using snix-eval instead of spawning
`nix eval`.  Eliminates process overhead for pure evaluation tasks and
enables fast flake introspection.

## Requirements

### NixEvalTool definition

r[nix.eval.tool-def]
The system MUST register a `NixEvalTool` with the following schema:

```json
{
  "name": "nix_eval",
  "description": "Evaluate a Nix expression in-process. Fast, no process spawn. Use for reading flake metadata, evaluating config values, listing available packages. Falls back to `nix eval` for impure expressions.",
  "input_schema": {
    "type": "object",
    "properties": {
      "expr": {
        "type": "string",
        "description": "Nix expression to evaluate"
      },
      "file": {
        "type": "string",
        "description": "Path to a .nix file to evaluate (alternative to expr)"
      },
      "apply": {
        "type": "string",
        "description": "Function to apply to the result (e.g., 'builtins.attrNames')"
      }
    },
    "required": []
  }
}
```

At least one of `expr` or `file` MUST be provided.

### Pure evaluation

r[nix.eval.pure]
The tool MUST evaluate expressions using `snix_eval::EvaluationBuilder`
in pure mode (no filesystem access, no environment variables, no network):

```rust
let result = EvaluationBuilder::new_pure()
    .build_result(&source, &path);
```

GIVEN the expression `1 + 1`
WHEN evaluated in pure mode
THEN the result is `2`

GIVEN the expression `builtins.attrNames { a = 1; b = 2; }`
WHEN evaluated in pure mode
THEN the result is `["a", "b"]`

### Impure fallback

r[nix.eval.impure-fallback]
When pure evaluation fails due to impure operations (file access, `<nixpkgs>`
lookup, fetchurl, IFD), the tool MUST fall back to spawning `nix eval --json`:

```rust
match evaluate_pure(&expr) {
    Ok(value) => format_value(value),
    Err(e) if e.is_impure() => fallback_nix_eval_cli(&expr),
    Err(e) => return ToolResult::error(e),
}
```

The fallback MUST be transparent to the agent — the result format is the
same regardless of which path executed.

GIVEN the expression `import ./default.nix`
WHEN pure evaluation fails (file import)
THEN the tool falls back to `nix eval --json --expr 'import ./default.nix'`
AND returns the JSON result

### Value serialization

r[nix.eval.serialize]
Nix values MUST be serialized to JSON for the tool result:

| Nix type | JSON type |
|---|---|
| int, float | number |
| string | string |
| bool | boolean |
| null | null |
| list | array |
| attrset | object |
| path | string (absolute path) |
| lambda | `"<lambda>"` |
| derivation (attrset) | object with `name`, `system`, `outPath` extracted |

The system MUST use `snix-serde` for serialization where possible.

### Flake introspection

r[nix.eval.flake-introspect]
The system MUST provide a convenience function for listing a flake's outputs
without spawning `nix flake show`:

```rust
pub fn introspect_flake(flake_dir: &Path) -> Result<FlakeOutputs, NixError>;

#[derive(Debug, Clone, Serialize)]
pub struct FlakeOutputs {
    pub packages: Vec<String>,
    pub dev_shells: Vec<String>,
    pub checks: Vec<String>,
    pub apps: Vec<String>,
    pub nixos_configurations: Vec<String>,
    pub other: Vec<String>,
}
```

This evaluates `builtins.attrNames` on each standard output type.

GIVEN a flake with `packages.x86_64-linux.default` and `devShells.x86_64-linux.default`
WHEN `introspect_flake` is called
THEN `packages` contains `"x86_64-linux.default"`
AND `dev_shells` contains `"x86_64-linux.default"`

r[nix.eval.flake-introspect-fallback]
If in-process flake evaluation fails (flakes require impure features for
lock resolution), the system MUST fall back to `nix flake show --json`.

### Evaluation limits

r[nix.eval.limits]
The system MUST enforce evaluation limits to prevent runaway expressions:

- Maximum evaluation steps: 1,000,000
- Maximum output size: 1 MB (JSON serialized)
- Timeout: 10 seconds for pure eval, 60 seconds for CLI fallback

GIVEN the expression `builtins.foldl' (x: y: x + y) 0 (builtins.genList (x: x) 999999999)`
WHEN the evaluation exceeds the step limit
THEN the tool returns an error: "Evaluation limit exceeded (1M steps)"

### Tool tier

r[nix.eval.tier]
The NixEvalTool MUST be registered at `ToolTier::Specialty`, same as NixTool.
It is only included in the agent's tool list when nix is detected on the
system.
