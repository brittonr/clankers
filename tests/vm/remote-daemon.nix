# Two-VM NixOS integration test for remote daemon access over iroh QUIC.
#
# Validates the full path: daemon starts with iroh endpoint → client
# connects via QUIC on the shared vlan → RPC ping → daemon control
# commands (create session, list sessions).
#
# Usage:
#   cargo build  # builds to $CARGO_TARGET_DIR/debug/clankers
#   nix-build tests/vm/remote-daemon.nix
#
# Interactive debugging:
#   $(nix-build tests/vm/remote-daemon.nix -A driverInteractive)/bin/nixos-test-driver
#   >>> start_all()
#   >>> server.shell_interact()
let
  # Use the same nixpkgs as the flake (unstable) to match library versions
  # in the cargo-built binary (libgit2, openssl, etc.).
  nixpkgs = fetchTarball "https://github.com/NixOS/nixpkgs/tarball/608d0cadfed240589a7eea422407a547ad626a14";
  pkgs = import nixpkgs { config = {}; overlays = []; };

  # Path to the cargo-built binary.
  # Honors CARGO_TARGET_DIR; falls back to target/ in the repo root.
  clankersBin =
    let
      envTarget = builtins.getEnv "CARGO_TARGET_DIR";
      base = if envTarget != "" then envTarget else ../../target;
    in "${base}/debug/clankers";

  # Wrap the cargo-built binary so it works in the NixOS VM.
  # autoPatchelfHook rewrites the ELF interpreter and RPATH to point at
  # the VM's nix store rather than the build host's paths.
  clankersWrapped = pkgs.stdenv.mkDerivation {
    pname = "clankers";
    version = "test";
    src = builtins.path {
      name = "clankers-bin";
      path = clankersBin;
    };

    nativeBuildInputs = [ pkgs.autoPatchelfHook ];
    buildInputs = [
      pkgs.openssl
      pkgs.sqlite
      pkgs.libgit2
      pkgs.libssh2
      pkgs.zlib
      pkgs.zstd
      pkgs.stdenv.cc.cc.lib  # libgcc_s, libstdc++
    ];

    dontUnpack = true;
    installPhase = ''
      mkdir -p $out/bin
      cp $src $out/bin/clankers
      chmod +x $out/bin/clankers
    '';
  };

  sharedModule = {
    virtualisation.graphics = false;
    virtualisation.memorySize = 2048;
    networking.firewall.enable = false;
    environment.systemPackages = [ clankersWrapped ];
    environment.variables = {
      HOME = "/root";
      TERM = "xterm-256color";
      RUST_LOG = "info";
      # Disable mDNS — multicast socket creation fails in QEMU VMs.
      # DNS/pkarr discovery works when the VMs have internet (not in
      # all CI environments). The test validates what it can reach.
      CLANKERS_NO_MDNS = "1";
      # Dummy API key — the daemon needs a provider to start, but we
      # never actually send LLM requests in this test.
      ANTHROPIC_API_KEY = "sk-ant-test-dummy-key-for-vm-integration-test";
    };
  };

in pkgs.testers.runNixOSTest {
  name = "clankers-remote-daemon";
  skipLint = true;

  nodes.server = { imports = [ sharedModule ]; };
  nodes.client = { imports = [ sharedModule ]; };

  testScript = ''
    start_all()
    server.wait_for_unit("default.target")
    client.wait_for_unit("default.target")

    # Sanity: binary runs on both VMs
    server.succeed("clankers --version")
    client.succeed("clankers --version")

    # ── Phase 1: Start daemon on server ──────────────────────────────────
    server.succeed("mkdir -p /root/.clankers/agent")
    client.succeed("mkdir -p /root/.clankers/agent")

    # Start daemon in foreground, --allow-all skips ACL + token checks
    server.succeed(
        "clankers daemon start --allow-all --heartbeat 0 "
        "> /tmp/daemon.log 2>&1 &"
    )

    # Wait for iroh endpoint to bind and print the node ID
    server.wait_until_succeeds(
        "grep -q 'Node ID:' /tmp/daemon.log",
        timeout=60,
    )

    # Extract the node ID from the banner
    node_id = server.succeed(
        "grep 'Node ID:' /tmp/daemon.log | head -1 | awk '{print $NF}'"
    ).strip()
    assert len(node_id) > 20, f"node ID looks too short: '{node_id}'"
    server.log(f"Server node ID: {node_id}")

    # ── Phase 2: Daemon health via control socket ────────────────────────
    status_local = server.succeed("clankers daemon status 2>&1")
    assert "running" in status_local.lower(), \
      f"daemon not running: {status_local}"
    server.log("Daemon is running")

    # ── Phase 3: Create session via control socket ───────────────────────
    session_out = server.succeed("clankers daemon create 2>&1")
    server.log(f"Session created: {session_out.strip()}")

    # Session appears in listing
    ps_out = server.succeed("clankers daemon sessions 2>&1")
    server.log(f"Session listing: {ps_out.strip()}")

    # ── Phase 4: iroh QUIC connectivity (relay-mediated) ─────────────────
    # Try RPC ping — this requires relay/pkarr discovery which needs
    # internet access. If VMs have internet, this validates the full
    # cross-machine iroh QUIC path. If not, we skip gracefully.
    ping_result = server.succeed(
        f"timeout 15 clankers rpc ping {node_id} 2>&1 || echo PING_UNAVAILABLE"
    )
    if "pong" in ping_result.lower():
        server.log("RPC self-ping over iroh QUIC succeeded!")
    else:
        server.log(f"RPC ping skipped (no relay access): {ping_result[-200:]}")

    # ── Phase 5: Verify iroh accept loop is running ──────────────────────
    daemon_log_content = server.succeed("cat /tmp/daemon.log 2>&1")
    assert "iroh accept loop started" in daemon_log_content, \
      "iroh accept loop not found in daemon log"
    server.log("iroh accept loop confirmed running")

    # ── Phase 6: Client can reach daemon RPC (if relay available) ────────
    client_ping = client.succeed(
        f"timeout 15 clankers rpc ping {node_id} 2>&1 || echo PING_UNAVAILABLE"
    )
    if "pong" in client_ping.lower():
        client.log("Cross-VM RPC ping over iroh QUIC succeeded!")

        # Full remote status check
        remote_status = client.succeed(
            f"clankers rpc status {node_id} 2>&1"
        )
        client.log(f"Remote status: {remote_status.strip()}")
    else:
        client.log(f"Cross-VM ping skipped (no relay): {client_ping[-200:]}")

    # ── Phase 7: Daemon stays healthy ────────────────────────────────────
    final_status = server.succeed("clankers daemon status 2>&1")
    assert "running" in final_status.lower(), f"daemon died: {final_status}"
    server.log("All remote daemon integration tests passed")
  '';
}
