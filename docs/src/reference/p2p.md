# P2P & Networking

Clankers uses [iroh](https://iroh.computer) for peer-to-peer communication over QUIC.

## RPC

Agent-to-agent communication:

```bash
clankers rpc id                         # show your node ID
clankers rpc start                      # start RPC server
clankers rpc ping <node-id>             # ping a remote instance
clankers rpc prompt <node-id> "..."     # send a prompt to a remote agent
clankers rpc send-file <node-id> <path> # send a file
clankers rpc peers list                 # list known peers
clankers rpc discover --mdns            # find peers on the LAN
```

## Remote daemon access

Attach to a daemon running on another machine:

```bash
clankers attach --remote <node-id>
```

Uses iroh QUIC with ALPN `clankers/daemon/1`.

## Session sharing

Share a live Zellij terminal session over the network:

```bash
clankers share                          # get a node ID + key
clankers join <node-id> <key>           # join from another machine
```

## TUI peer management

```
/peers                          # list all peers
/peers add <node-id> <name>     # add a peer
/peers remove <name-or-id>      # remove a peer
/peers probe [name-or-id]       # probe connectivity
/peers discover                 # scan LAN via mDNS
/peers allow <node-id>          # add to allowlist
/peers deny <node-id>           # remove from allowlist
/peers server [on|off]          # start/stop embedded RPC server
```

## Matrix bridge

Multi-agent coordination over encrypted Matrix channels. Instances exchange structured messages (`m.clankers.*` types).

```bash
clankers daemon start --matrix
```

Matrix tools available to the agent:

| Tool | Purpose |
|------|---------|
| `matrix_send` | Send a message to a room |
| `matrix_read` | Read messages from a room |
| `matrix_rooms` | List joined rooms |
| `matrix_peers` | List known agents |
| `matrix_join` | Join a room |
| `matrix_rpc` | Send an RPC request to another agent |
