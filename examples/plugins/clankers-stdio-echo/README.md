# clankers-stdio-echo

Minimal reference stdio plugin.

Covers real host/plugin framing for:
- hello / ready handshake
- live tool registration
- tool invocation
- tool cancellation
- orderly shutdown

## Tool

`stdio_echo_fixture`

Input:
- `{"mode":"echo","message":"hi"}` → returns `fixture:hi`
- `{"mode":"wait_for_cancel"}` → waits for `tool_cancel`, then returns `tool_cancelled`

## Run

Drop this directory into a scanned plugin root, or install it with:

```bash
clankers plugin install examples/plugins/clankers-stdio-echo
```

`plugin.json` uses:
- `kind: "stdio"`
- `command: "./plugin.py"`
- `working_dir: "plugin-dir"`
- `sandbox: "inherit"`
