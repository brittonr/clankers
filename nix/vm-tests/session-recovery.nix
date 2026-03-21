# Session recovery: create → stop → restart → verify suspended sessions
# survive, then crash recovery (SIGKILL → restart).
{ pkgs, clankersPkg }:
pkgs.testers.runNixOSTest {
  name = "clankers-session-recovery";
  skipLint = true;

  nodes.machine = { pkgs, ... }: {
    virtualisation.graphics = false;
    virtualisation.memorySize = 2048;
    environment.systemPackages = [ clankersPkg pkgs.jq ];
    environment.variables = {
      HOME = "/root";
      TERM = "xterm-256color";
      RUST_LOG = "info";
    };
  };

  testScript = ''
    import time

    machine.wait_for_unit("default.target")
    machine.succeed("mkdir -p /root/.clankers/agent")

    # ── Phase 1: Start daemon and create sessions ────────────────────
    machine.succeed(
        "clankers daemon start --heartbeat 0 "
        "> /tmp/daemon1.log 2>&1 &"
    )
    machine.wait_until_succeeds(
        "clankers daemon status | grep -q 'Daemon running'",
        timeout=30,
    )
    machine.log("Daemon started")

    machine.succeed("clankers daemon create > /tmp/s1.out 2>&1")
    session1 = machine.succeed("cat /tmp/s1.out").strip()
    machine.log(f"Session 1: {session1}")

    machine.succeed("clankers daemon create > /tmp/s2.out 2>&1")
    session2 = machine.succeed("cat /tmp/s2.out").strip()
    machine.log(f"Session 2: {session2}")

    ps_out = machine.succeed("clankers ps")
    machine.log(f"ps output:\n{ps_out}")
    assert "active" in ps_out, f"expected active sessions in ps output"

    status_out = machine.succeed("clankers daemon status")
    assert "Sessions: 2" in status_out, f"expected 2 sessions: {status_out}"

    # ── Phase 2: Stop daemon (checkpoint) ────────────────────────────
    machine.succeed("clankers daemon stop")
    machine.wait_until_succeeds(
        "! clankers daemon status 2>&1 | grep -q 'Daemon running'",
        timeout=15,
    )
    machine.log("Daemon stopped — sessions checkpointed")

    machine.succeed("test -f /root/.clankers/agent/clankers.db")

    # ── Phase 3: Restart daemon ──────────────────────────────────────
    machine.succeed(
        "clankers daemon start --heartbeat 0 "
        "> /tmp/daemon2.log 2>&1 &"
    )
    machine.wait_until_succeeds(
        "clankers daemon status | grep -q 'Daemon running'",
        timeout=30,
    )
    machine.log("Daemon restarted")

    # ── Phase 4: Verify suspended sessions recovered ─────────────────
    ps_out = machine.succeed("clankers ps")
    machine.log(f"ps after restart:\n{ps_out}")
    assert "suspended" in ps_out, \
        f"expected suspended sessions after restart: {ps_out}"

    status_out = machine.succeed("clankers daemon status")
    machine.log(f"status after restart:\n{status_out}")
    assert "Suspended" in status_out, \
        f"expected Suspended count in status: {status_out}"

    # ── Phase 5: Crash recovery (SIGKILL, no clean shutdown) ─────────
    machine.succeed(
        "kill -9 $(cat /run/user/0/clankers/daemon.pid) || true"
    )
    machine.sleep(2)

    machine.succeed(
        "clankers daemon start --heartbeat 0 "
        "> /tmp/daemon3.log 2>&1 &"
    )
    machine.wait_until_succeeds(
        "clankers daemon status | grep -q 'Daemon running'",
        timeout=30,
    )

    ps_out = machine.succeed("clankers ps")
    machine.log(f"ps after crash recovery:\n{ps_out}")
    assert "suspended" in ps_out, \
        f"expected suspended sessions after crash recovery: {ps_out}"

    # ── Phase 6: Clean stop ──────────────────────────────────────────
    machine.succeed("clankers daemon stop")
    machine.log("Session recovery test passed")
  '';
}
