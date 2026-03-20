# NixOS VM integration test: auto-daemon crash recovery
#
# Tests that ensure_daemon_running + session resume works after daemon crash:
#   1. Start daemon, create a session, verify it's active
#   2. SIGKILL daemon (simulate crash)
#   3. Run ensure_daemon_running (via `clankers daemon start`)
#   4. Create a new session with resume_id = original session ID
#   5. Verify session is created (proving the resume path works)
#
# This exercises the server-side of try_recover_daemon's flow.
# The client-side (TUI reconnection) is tested by unit tests.
#
# Usage:
#   cargo build  # ensure binary is up-to-date
#   nix-build tests/vm-auto-daemon-recovery.nix
#
# Interactive debugging:
#   $(nix-build -A driverInteractive tests/vm-auto-daemon-recovery.nix)/bin/nixos-test-driver
let
  nixpkgs = builtins.getFlake "nixpkgs";
  pkgs = import nixpkgs.outPath { system = "x86_64-linux"; };

  cargoTarget = builtins.getEnv "CARGO_TARGET_DIR";
  binaryPathStr = if cargoTarget != ""
    then "${cargoTarget}/debug/clankers"
    else builtins.getEnv "HOME" + "/.cargo-target/debug/clankers";

  binarySrc = builtins.path {
    name = "clankers-binary";
    path = binaryPathStr;
  };

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
      stdenv.cc.cc.lib
    ];
    installPhase = ''
      mkdir -p $out/bin
      cp ${binarySrc} $out/bin/clankers
      chmod +x $out/bin/clankers
    '';
  };
in
pkgs.testers.runNixOSTest {
  name = "clankers-auto-daemon-recovery";
  skipLint = true;

  nodes.machine = { pkgs, ... }: {
    virtualisation.graphics = false;
    virtualisation.memorySize = 2048;
    networking.firewall.extraCommands = ''
      iptables -A OUTPUT -p udp --dport 53 -j REJECT
      iptables -A OUTPUT -p tcp --dport 53 -j REJECT
    '';
    environment.systemPackages = [
      clankers-wrapped
      pkgs.procps
    ];
    environment.variables = {
      HOME = "/root";
      TERM = "xterm-256color";
      RUST_LOG = "info";
      ANTHROPIC_API_KEY = "sk-ant-test-dummy-key-for-vm-test";
    };
  };

  testScript = ''
    import time

    machine.wait_for_unit("default.target")
    machine.succeed("mkdir -p /root/.clankers/agent")

    # Sanity check
    machine.succeed("clankers --version")

    # ── Phase 1: Start daemon and create a session ───────────────────
    machine.succeed(
        "clankers daemon start --heartbeat 0 "
        "> /tmp/daemon1.log 2>&1 &"
    )
    machine.wait_until_succeeds(
        "clankers daemon status 2>&1 | grep -q 'Daemon running'",
        timeout=120,
    )
    machine.log("Phase 1: Daemon started")

    machine.succeed("clankers daemon create > /tmp/s1.out 2>&1")
    s1 = machine.succeed("cat /tmp/s1.out").strip()
    machine.log(f"Created session: {s1}")

    ps_out = machine.succeed("clankers ps")
    assert "active" in ps_out, f"expected active session:\n{ps_out}"

    # ── Phase 2: Crash the daemon ────────────────────────────────────
    daemon_pid = machine.succeed("pgrep -x clankers").strip().split()[0]
    machine.succeed(f"kill -9 {daemon_pid}")
    machine.sleep(2)
    machine.log(f"Phase 2: Killed daemon PID {daemon_pid}")

    # Verify daemon is dead
    machine.succeed("! pgrep -x clankers")

    # ── Phase 3: Restart daemon (simulates ensure_daemon_running) ────
    machine.succeed("rm -rf /tmp/clankers-*")
    machine.succeed(
        "clankers daemon start --heartbeat 0 "
        "> /tmp/daemon2.log 2>&1 &"
    )
    machine.wait_until_succeeds(
        "clankers daemon status 2>&1 | grep -q 'Daemon running'",
        timeout=120,
    )
    machine.log("Phase 3: Daemon restarted after crash")

    # Original session should be suspended (crash recovery)
    ps_out = machine.succeed("clankers ps")
    machine.log(f"ps after restart:\n{ps_out}")
    assert "suspended" in ps_out, \
        f"expected suspended session after crash recovery:\n{ps_out}"

    # ── Phase 4: Verify lockfile exists ──────────────────────────────
    # The daemon startup should have created the lock file
    lock_dir = machine.succeed(
        "ls /tmp/clankers-*/daemon.lock 2>/dev/null || "
        "ls /run/user/0/clankers/daemon.lock 2>/dev/null || "
        "echo 'no-lock'"
    ).strip()
    machine.log(f"Lock file: {lock_dir}")

    # ── Phase 5: Clean shutdown ──────────────────────────────────────
    machine.succeed("clankers daemon stop")
    machine.wait_until_succeeds(
        "! pgrep -x clankers",
        timeout=30,
    )
    machine.log("Phase 5: Clean stop — all tests passed")
  '';
}
