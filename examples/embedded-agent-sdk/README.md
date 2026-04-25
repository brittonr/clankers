# Embedded Agent SDK example

Standalone consumer fixture for the reusable Clankers engine crates. It is intentionally outside the root workspace and depends only on:

- `clanker-message`
- `clankers-engine`
- `clankers-engine-host`
- `clankers-tool-host`
- `serde_json` for host-owned tool payloads

Run it from the repository root:

```bash
cargo run --manifest-path examples/embedded-agent-sdk/Cargo.toml
```

The binary executes positive and negative adapter paths for model execution, tool execution, retry sleeping, event emission, cancellation, usage observation, and host-owned transcript conversion.
