# NixOS module test: services.clankers-daemon
#
# Validates the clankers-daemon NixOS module:
#   - systemd unit starts and stays running
#   - system user/group created
#   - state directory exists with correct ownership
#   - hardening directives applied (NoNewPrivileges, PrivateTmp)
#   - service restarts on failure
#   - environment file support
{ pkgs, clankersPkg, clankersDaemonModule }:
pkgs.testers.runNixOSTest {
  name = "clankers-module-daemon";
  skipLint = true;

  nodes.machine = { pkgs, lib, ... }: {
    imports = [ clankersDaemonModule ];

    virtualisation.graphics = false;
    virtualisation.memorySize = 2048;

    # The daemon needs network for iroh, but we don't need real DNS
    networking.firewall.enable = false;

    # Write a fake env file with a dummy API key so the daemon
    # starts its provider chain without real credentials.
    environment.etc."clankers-env".text = ''
      ANTHROPIC_API_KEY=sk-ant-test-dummy-key-for-vm-test
    '';

    services.clankers-daemon = {
      enable = true;
      package = clankersPkg;
      model = "claude-sonnet-4-20250514";
      heartbeat = 0;  # disable heartbeat in test
      allowAll = true;
      environmentFile = "/etc/clankers-env";
      processManagement = {
        enable = true;
        defaultBackend = "native";
        databasePath = "/var/lib/clankers/process-jobs/process-jobs.redb";
        registryDir = "/var/lib/clankers/process-jobs";
        logDir = "/var/log/clankers/process-jobs";
        retention = {
          maxAgeDays = 7;
          maxRecords = 250;
          maxLogBytes = 10485760;
        };
      };
    };
  };

  testScript = ''
    machine.wait_for_unit("default.target")

    # ── Phase 1: User/group creation ─────────────────────────────────
    machine.succeed("getent passwd clankers")
    machine.succeed("getent group clankers")

    # Verify it's a system user (UID < 1000 on NixOS)
    uid = int(machine.succeed("id -u clankers").strip())
    assert uid < 1000, f"expected system user, got UID {uid}"
    machine.log(f"clankers user UID: {uid}")

    # ── Phase 2: State directory ─────────────────────────────────────
    machine.succeed("test -d /var/lib/clankers")
    owner = machine.succeed("stat -c '%U:%G' /var/lib/clankers").strip()
    assert owner == "clankers:clankers", f"bad ownership: {owner}"
    machine.log("State directory exists with correct ownership")

    # ── Phase 3: Process-job persistence directories ─────────────────
    machine.succeed("test -d /var/lib/clankers/process-jobs")
    machine.succeed("test -d /var/log/clankers/process-jobs")
    job_owner = machine.succeed("stat -c '%U:%G %a' /var/lib/clankers/process-jobs").strip()
    log_owner = machine.succeed("stat -c '%U:%G %a' /var/log/clankers/process-jobs").strip()
    assert job_owner == "clankers:clankers 750", f"bad job dir ownership/mode: {job_owner}"
    assert log_owner == "clankers:clankers 750", f"bad log dir ownership/mode: {log_owner}"

    # ── Phase 4: Service starts ──────────────────────────────────────
    # The daemon takes a moment to bind its iroh endpoint; wait for
    # the unit to reach active state (or at least be started).
    machine.wait_for_unit("clankers-daemon.service", timeout=60)
    machine.log("clankers-daemon.service is active")

    # Verify it's actually running
    machine.succeed("systemctl is-active clankers-daemon.service")

    # Check the process is running as the clankers user
    ps_user = machine.succeed(
        "ps -o user= -C clankers | head -1"
    ).strip()
    assert ps_user == "clankers", f"expected clankers user, got: {ps_user}"
    machine.log("Daemon process running as clankers user")

    # ── Phase 5: Systemd hardening ───────────────────────────────────
    props = machine.succeed(
        "systemctl show clankers-daemon.service "
        "--property=NoNewPrivileges,PrivateTmp,ProtectSystem,ReadWritePaths"
    )
    machine.log(f"Service properties:\n{props}")
    assert "NoNewPrivileges=yes" in props, f"NoNewPrivileges not set: {props}"
    assert "PrivateTmp=yes" in props, f"PrivateTmp not set: {props}"
    assert "ProtectSystem=strict" in props, f"ProtectSystem not strict: {props}"
    assert "/var/lib/clankers/process-jobs" in props, f"process job registry path not writable: {props}"
    assert "/var/log/clankers/process-jobs" in props, f"process job log path not writable: {props}"

    # ── Phase 6: Process-job environment configured ──────────────────
    env_props = machine.succeed(
        "systemctl show clankers-daemon.service --property=Environment"
    )
    assert "CLANKERS_PROCESS_JOBS_ENABLED=1" in env_props, env_props
    assert "CLANKERS_PROCESS_JOB_DEFAULT_BACKEND=native" in env_props, env_props
    assert "CLANKERS_PROCESS_JOB_DB=/var/lib/clankers/process-jobs/process-jobs.redb" in env_props, env_props
    assert "CLANKERS_PROCESS_JOB_LOG_DIR=/var/log/clankers/process-jobs" in env_props, env_props
    assert "CLANKERS_PROCESS_JOB_RETENTION_MAX_AGE_DAYS=7" in env_props, env_props

    # ── Phase 7: Environment file loaded ─────────────────────────────
    env_file_prop = machine.succeed(
        "systemctl show clankers-daemon.service --property=EnvironmentFiles"
    ).strip()
    assert "/etc/clankers-env" in env_file_prop, \
        f"EnvironmentFile not set: {env_file_prop}"
    machine.log("EnvironmentFile configured correctly")

    # ── Phase 6: Service restart on failure ──────────────────────────
    restart_prop = machine.succeed(
        "systemctl show clankers-daemon.service --property=Restart"
    ).strip()
    assert "on-failure" in restart_prop, f"Restart policy wrong: {restart_prop}"

    # Kill the daemon and verify systemd restarts it
    machine.succeed("systemctl kill --signal=KILL clankers-daemon.service")
    machine.sleep(8)  # RestartSec=5 + buffer
    machine.wait_for_unit("clankers-daemon.service", timeout=30)
    machine.log("Service restarted after SIGKILL")

    # ── Phase 7: Clean stop ──────────────────────────────────────────
    machine.succeed("systemctl stop clankers-daemon.service")
    machine.succeed("! systemctl is-active clankers-daemon.service")
    machine.log("Module daemon test passed")
  '';
}
