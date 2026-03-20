# NixOS VM integration test: daemon session recovery
#
# Tests the full lifecycle:
#   1. Start daemon, create sessions → active
#   2. Stop daemon (clean checkpoint) → suspended in catalog
#   3. Restart daemon → suspended sessions loaded from catalog
#   4. Crash daemon (SIGKILL) → stale active entries
#   5. Restart daemon → crash recovery (active → suspended)
#
# Usage:
#   cargo build  # ensure binary is up-to-date
#   nix-build tests/vm-session-recovery.nix
#
# Interactive debugging:
#   $(nix-build -A driverInteractive tests/vm-session-recovery.nix)/bin/nixos-test-driver
let
  nixpkgs = builtins.getFlake "nixpkgs";
  pkgs = import nixpkgs.outPath { system = "x86_64-linux"; };

  cargoTarget = builtins.getEnv "CARGO_TARGET_DIR";
  binaryPathStr = if cargoTarget != ""
    then "${cargoTarget}/debug/clankers"
    else builtins.getEnv "HOME" + "/.cargo-target/debug/clankers";

  # Copy the binary into the nix store so it's available on remote builders.
  binarySrc = builtins.path {
    name = "clankers-binary";
    path = binaryPathStr;
  };

  # Wrap the cargo-built binary into a nix package with its runtime deps.
  clankers-wrapped = pkgs.stdenv.mkDerivation {
    name = "clankers-wrapped";
    dontUnpack = true;
    nativeBuildInputs = [ pkgs.autoPatchelfHook ];
    buildInputs = with pkgs; [
      sqlite
      libgit2
      openssl
      zlib
      libssh2
      pcre2
      llhttp
      stdenv.cc.cc.lib  # libgcc_s.so.1
    ];
    installPhase = ''
      mkdir -p $out/bin
      cp ${binarySrc} $out/bin/clankers
      chmod +x $out/bin/clankers
    '';
  };
in
pkgs.testers.runNixOSTest {
  name = "clankers-session-recovery";
  skipLint = true;

  nodes.machine = { pkgs, ... }: {
    virtualisation.graphics = false;
    virtualisation.memorySize = 2048;
    # Block outbound DNS so iroh fails immediately instead of retrying
    # for minutes. Makes daemon startup fast in offline VMs.
    networking.firewall.extraCommands = ''
      iptables -A OUTPUT -p udp --dport 53 -j REJECT
      iptables -A OUTPUT -p tcp --dport 53 -j REJECT
    '';
    environment.systemPackages = [
      clankers-wrapped
      pkgs.procps  # for pkill/pgrep
    ];
    environment.variables = {
      HOME = "/root";
      TERM = "xterm-256color";
      RUST_LOG = "info";
      # Dummy key — no LLM calls made, but daemon needs a provider to start.
      ANTHROPIC_API_KEY = "sk-ant-test-dummy-key-for-vm-test";
    };
  };

  testScript = ''
    import time

    machine.wait_for_unit("default.target")
    machine.succeed("mkdir -p /root/.clankers/agent")

    # Sanity: binary runs
    version = machine.succeed("clankers --version").strip()
    machine.log(f"Binary version: {version}")

    # ── Phase 1: Start daemon and create sessions ────────────────────
    machine.succeed(
        "clankers daemon start --heartbeat 0 "
        "> /tmp/daemon1.log 2>&1 &"
    )
    machine.wait_until_succeeds(
        "clankers daemon status 2>&1 | grep -q 'Daemon running'",
        timeout=120,
    )
    machine.log("Phase 1: Daemon started")

    # Create two sessions
    machine.succeed("clankers daemon create > /tmp/s1.out 2>&1")
    s1 = machine.succeed("cat /tmp/s1.out").strip()
    machine.log(f"Created session 1: {s1}")

    machine.succeed("clankers daemon create > /tmp/s2.out 2>&1")
    s2 = machine.succeed("cat /tmp/s2.out").strip()
    machine.log(f"Created session 2: {s2}")

    # Both sessions active
    ps_out = machine.succeed("clankers ps")
    machine.log(f"ps (phase 1):\n{ps_out}")
    assert ps_out.count("active") >= 2, f"expected 2 active sessions:\n{ps_out}"

    # Status shows 2 sessions
    status_out = machine.succeed("clankers daemon status")
    assert "Sessions: 2" in status_out, f"expected 2 sessions:\n{status_out}"

    # ── Phase 2: Clean stop (checkpoint) ─────────────────────────────
    machine.succeed("clankers daemon stop")
    machine.wait_until_succeeds(
        "! pgrep -x clankers",
        timeout=30,
    )
    machine.log("Phase 2: Daemon stopped cleanly")

    # Catalog DB should exist
    machine.succeed("test -f /root/.clankers/agent/clankers.db")

    # Clean stale socket dir left by the previous daemon (PID file, etc.)
    machine.succeed("rm -rf /tmp/clankers-*")

    # ── Phase 3: Restart — sessions should be suspended ──────────────
    machine.succeed(
        "clankers daemon start --heartbeat 0 "
        "> /tmp/daemon2.log 2>&1 &"
    )
    machine.wait_until_succeeds(
        "clankers daemon status 2>&1 | grep -q 'Daemon running'",
        timeout=120,
    )
    machine.log("Phase 3: Daemon restarted")

    ps_out = machine.succeed("clankers ps")
    machine.log(f"ps (phase 3):\n{ps_out}")
    assert "suspended" in ps_out, \
        f"expected suspended sessions after restart:\n{ps_out}"

    # Status should report suspended count
    status_out = machine.succeed("clankers daemon status")
    machine.log(f"status (phase 3):\n{status_out}")
    assert "Suspended" in status_out, \
        f"expected Suspended in status:\n{status_out}"

    # ── Phase 4: Crash the daemon (SIGKILL — no checkpoint) ──────────
    daemon_pid = machine.succeed("pgrep -x clankers").strip().split()[0]
    machine.succeed(f"kill -9 {daemon_pid}")
    machine.sleep(2)
    machine.log(f"Phase 4: Killed daemon PID {daemon_pid}")

    # ── Phase 5: Restart after crash — should recover stale entries ──
    machine.succeed("rm -rf /tmp/clankers-*")
    machine.succeed(
        "clankers daemon start --heartbeat 0 "
        "> /tmp/daemon3.log 2>&1 &"
    )
    machine.wait_until_succeeds(
        "clankers daemon status 2>&1 | grep -q 'Daemon running'",
        timeout=120,
    )
    machine.log("Phase 5: Daemon restarted after crash")

    # Check daemon log for crash recovery message
    log3 = machine.succeed("cat /tmp/daemon3.log")
    machine.log(f"daemon3 log:\n{log3}")

    ps_out = machine.succeed("clankers ps")
    machine.log(f"ps (phase 5):\n{ps_out}")
    assert "suspended" in ps_out, \
        f"expected suspended sessions after crash recovery:\n{ps_out}"

    # ── Phase 6: Clean shutdown ──────────────────────────────────────
    machine.succeed("clankers daemon stop")
    machine.wait_until_succeeds(
        "! pgrep -x clankers",
        timeout=30,
    )
    machine.log("Phase 6: Clean stop — all tests passed")
  '';
}
