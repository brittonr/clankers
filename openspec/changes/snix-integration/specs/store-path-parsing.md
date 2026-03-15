# Store Path Parsing

## Purpose

Parse nix store paths from CLI output into typed structures the agent can
reason about.  Replace regex/string matching with nix-compat's StorePath.

## Requirements

### NixPath wrapper type

r[nix.store-path.wrapper]
The system MUST define a `NixPath` struct wrapping parsed store path data:

```rust
/// Agent-friendly representation of a nix store path.
#[derive(Debug, Clone, Serialize)]
pub struct NixPath {
    /// Full absolute path (e.g., "/nix/store/abc123-hello-2.12.1")
    pub path: String,
    /// The name component (e.g., "hello-2.12.1")
    pub name: String,
    /// The nixbase32-encoded hash (e.g., "abc123...")
    pub store_hash: String,
    /// Whether this is a .drv file
    pub is_derivation: bool,
}
```

### Parsing from absolute paths

r[nix.store-path.parse-absolute]
The system MUST parse absolute store paths using `nix_compat::store_path::StorePath`:

```rust
pub fn parse_store_path(path: &str) -> Result<NixPath, NixError>;
```

GIVEN a string "/nix/store/ql5gvvahh5gnir9g8v25xd4dwqa4hcmp-hello-2.12.1"
WHEN parsed via `parse_store_path`
THEN it returns `NixPath { name: "hello-2.12.1", store_hash: "ql5gvv...", is_derivation: false }`

GIVEN a string "/nix/store/abc123-hello-2.12.1.drv"
WHEN parsed via `parse_store_path`
THEN `is_derivation` is `true`

GIVEN a string "/home/user/project"
WHEN parsed via `parse_store_path`
THEN it returns `NixError::NotAStorePath`

### Extracting store paths from build output

r[nix.store-path.extract]
The system MUST extract all store paths from a block of text:

```rust
pub fn extract_store_paths(text: &str) -> Vec<NixPath>;
```

This scans for `/nix/store/<hash>-<name>` patterns and parses each one.
Invalid matches (hash too short, malformed) are silently skipped.

GIVEN text containing "building '/nix/store/abc-foo.drv'...\n/nix/store/xyz-foo"
WHEN `extract_store_paths` is called
THEN it returns two NixPaths: the .drv and the output

### Structured build result

r[nix.store-path.build-result]
The NixTool MUST return parsed store paths in its result alongside raw output:

```json
{
  "exit_code": 0,
  "outputs": [
    {
      "path": "/nix/store/ql5gvv...-hello-2.12.1",
      "name": "hello-2.12.1",
      "store_hash": "ql5gvv...",
      "is_derivation": false
    }
  ],
  "build_log": "...",
  "messages": ["fetched hello-2.12.1 from cache"]
}
```

The `outputs` field contains only the final build output paths (lines
printed to stdout after a successful build), not intermediate derivation
paths from the build log.

### nixbase32 encoding/decoding

r[nix.store-path.nixbase32]
The system MUST use `nix_compat::nixbase32` for hash encoding and decoding
rather than reimplementing the non-standard base32 alphabet.
