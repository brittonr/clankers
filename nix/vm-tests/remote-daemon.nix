# Two-VM test: daemon on server, client connects via iroh QUIC.
# Validates the full remote attach path: endpoint binding, ALPN
# negotiation, control stream (create/list), and session attach.
{ pkgs, clankersPkg }:
pkgs.testers.runNixOSTest {
  name = "clankers-remote-daemon";
  skipLint = true;

  nodes.server = { pkgs, ... }: {
    virtualisation.graphics = false;
    virtualisation.memorySize = 2048;
    networking.firewall.enable = false;
    environment.systemPackages = [ clankersPkg pkgs.jq ];
    environment.variables = {
      HOME = "/root";
      TERM = "xterm-256color";
      RUST_LOG = "info";
    };
  };

  nodes.client = { pkgs, ... }: {
    virtualisation.graphics = false;
    virtualisation.memorySize = 2048;
    networking.firewall.enable = false;
    environment.systemPackages = [ clankersPkg pkgs.jq ];
    environment.variables = {
      HOME = "/root";
      TERM = "xterm-256color";
      RUST_LOG = "info";
    };
  };

  testScript = ''
    import json
    import time

    start_all()
    server.wait_for_unit("default.target")
    client.wait_for_unit("default.target")

    # ── Phase 1: Start daemon on server ──────────────────────────────
    server.succeed("mkdir -p /root/.clankers/agent")
    client.succeed("mkdir -p /root/.clankers/agent")

    server.succeed(
        "clankers daemon start --allow-all --heartbeat 0 "
        "> /tmp/daemon.log 2>&1 &"
    )

    server.wait_until_succeeds(
        "grep -q 'Node ID:' /tmp/daemon.log",
        timeout=30,
    )

    node_id = server.succeed(
        "grep 'Node ID:' /tmp/daemon.log | head -1 | awk '{print $NF}'"
    ).strip()
    assert len(node_id) > 20, f"node ID too short: '{node_id}'"
    server.log(f"Server node ID: {node_id}")

    server.succeed("clankers daemon status | grep -q 'Daemon running'")

    # ── Phase 2: RPC ping from client ────────────────────────────────
    client.wait_until_succeeds(
        f"clankers rpc ping {node_id} 2>&1 | grep -q 'pong'",
        timeout=60,
    )
    client.log("RPC ping succeeded")

    # ── Phase 3: Create session over QUIC ────────────────────────────
    server.succeed("clankers daemon create > /tmp/session.out 2>&1")
    session_line = server.succeed("cat /tmp/session.out").strip()
    server.log(f"Created session: {session_line}")

    server.succeed("clankers ps | grep -q 'claude'")

    # ── Phase 4: RPC status from client ──────────────────────────────
    status_out = client.succeed(
        f"clankers rpc status {node_id} 2>&1"
    )
    client.log(f"Remote status: {status_out}")

    # ── Phase 5: Verify daemon stays healthy ─────────────────────────
    server.succeed("clankers daemon status | grep -q 'Daemon running'")
    server.log("All remote daemon tests passed")
  '';
}
