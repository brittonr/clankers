# Flake Reference Validation

## Purpose

Validate and parse flake references before spawning the nix CLI.  Catch
malformed refs early with actionable errors instead of waiting for nix
to fail with cryptic messages.

## Requirements

### Flake ref parsing

r[nix.flakeref.parse]
The system MUST parse flake references using `nix_compat::flakeref::FlakeRef`:

```rust
pub fn parse_flake_ref(input: &str) -> Result<ParsedFlakeRef, NixError>;
```

The function parses the input into a typed `FlakeRef` variant (Git, GitHub,
Path, etc.) and extracts the fragment (attribute path) if present.

### ParsedFlakeRef type

r[nix.flakeref.typed]
The system MUST return a typed representation:

```rust
#[derive(Debug, Clone)]
pub struct ParsedFlakeRef {
    /// The source type (path, git, github, etc.)
    pub source_type: FlakeSourceType,
    /// The attribute path fragment (e.g., "packages.x86_64-linux.hello")
    pub fragment: Option<String>,
    /// Original input string
    pub raw: String,
}

#[derive(Debug, Clone)]
pub enum FlakeSourceType {
    Path,
    Git { url: String },
    GitHub { owner: String, repo: String },
    Tarball { url: String },
    Indirect { id: String },
    Other(String),
}
```

### Validation before CLI dispatch

r[nix.flakeref.validate-before-spawn]
The NixTool MUST validate flake reference arguments before spawning the
nix CLI process.  This applies to subcommands that accept flake refs:
`build`, `run`, `develop`, `shell`, `eval`, `flake show`, `flake check`.

GIVEN the agent calls NixTool with args `[".#nonexistent-output"]`
WHEN the flake ref parses successfully as a path ref with fragment
THEN the nix CLI is spawned normally (output existence can only be
checked by nix itself)

GIVEN the agent calls NixTool with args `["github:///malformed"]`
WHEN the flake ref fails to parse
THEN the tool returns an error immediately without spawning nix
AND the error message includes what's wrong with the reference

### Fragment extraction for introspection

r[nix.flakeref.fragment]
When a flake ref has a fragment (attribute path after `#`), the system
MUST extract it as a dot-separated path for use in evaluation and
error messages.

GIVEN input ".#packages.x86_64-linux.hello"
WHEN parsed
THEN `fragment` is `Some("packages.x86_64-linux.hello")`

GIVEN input "github:NixOS/nixpkgs"
WHEN parsed
THEN `fragment` is `None`

### Flake detection in project context

r[nix.flakeref.detect]
The system MUST provide a helper to detect whether the current working
directory is a flake project:

```rust
pub fn detect_flake(cwd: &Path) -> Option<FlakeInfo>;

pub struct FlakeInfo {
    pub flake_path: PathBuf,
    pub has_lock: bool,
}
```

GIVEN a directory containing `flake.nix`
WHEN `detect_flake` is called
THEN it returns `Some(FlakeInfo { ... })`

GIVEN a directory without `flake.nix`
WHEN `detect_flake` is called
THEN it returns `None`
