# Derivation Reading

## Purpose

Parse `.drv` files to extract build metadata the agent can use to understand
what a derivation does, what it depends on, and what it produces.  Replaces
opaque derivation paths with structured build plans.

## Requirements

### Derivation parsing

r[nix.derivation.parse]
The system MUST parse `.drv` files using `nix_compat::derivation::Derivation`:

```rust
pub fn read_derivation(drv_path: &Path) -> Result<DerivationInfo, NixError>;
```

### DerivationInfo type

r[nix.derivation.info-type]
The parsed derivation MUST be wrapped in an agent-friendly struct:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct DerivationInfo {
    /// Name of the derivation (e.g., "hello-2.12.1")
    pub name: String,
    /// Builder program (e.g., "/nix/store/...-bash-5.2/bin/bash")
    pub builder: String,
    /// Build system (e.g., "x86_64-linux")
    pub system: String,
    /// Named outputs and their paths
    pub outputs: Vec<OutputInfo>,
    /// Input derivations (direct build dependencies)
    pub input_drvs: Vec<InputDrvInfo>,
    /// Input sources (non-derivation inputs, e.g., source tarballs)
    pub input_srcs: Vec<String>,
    /// Environment variables set during build (filtered)
    pub build_env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputInfo {
    pub name: String,
    pub path: String,
    pub hash_algo: Option<String>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputDrvInfo {
    pub path: String,
    pub name: String,
    pub requested_outputs: Vec<String>,
}
```

### Environment filtering

r[nix.derivation.env-filter]
The `build_env` field MUST filter out large or noisy environment variables.
Include only variables useful for understanding the build:

- `name`, `version`, `pname`, `system`, `src`, `out`
- `buildInputs`, `nativeBuildInputs`, `propagatedBuildInputs`
- `configureFlags`, `cmakeFlags`, `mesonFlags`
- `buildPhase`, `installPhase`, `checkPhase` (truncated to 500 chars)
- `meta` (if present)

Exclude:
- `__sandboxProfile`, `__impureHostDeps`
- Variables whose values exceed 2000 characters (unless in the include list)
- `passthru*` variables

### Derivation from build output

r[nix.derivation.from-build]
The NixTool MAY read the `.drv` file for a build output when the agent
requests verbose output or when a build fails.  This is opt-in via a
`verbose` flag on the tool.

GIVEN a failed `nix build .#hello`
WHEN the build log references `/nix/store/abc-hello-2.12.1.drv`
AND the `.drv` file exists locally
THEN the tool MAY parse it and include `DerivationInfo` in the error response
to help the agent understand what was being built

### Input dependency summary

r[nix.derivation.dep-summary]
The system MUST provide a function to summarize a derivation's dependency
tree to a bounded depth:

```rust
pub fn dependency_summary(drv_path: &Path, max_depth: usize) -> Result<String, NixError>;
```

This produces a human-readable tree like:

```
hello-2.12.1
├── bash-5.2-p26
├── gcc-13.3.0 (cc)
├── glibc-2.38
└── hello-2.12.1-src (source)
```

Max depth defaults to 2 to keep output bounded.
