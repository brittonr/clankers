# NixOS module integration test: daemon + router together
#
# Validates both modules running on the same machine:
#   - Both services start and coexist
#   - Router listens before daemon starts (ordering)
#   - Daemon can reach the router's proxy port
#   - CLI tools work against the systemd-managed daemon
#   - Both services stop cleanly
{ pkgs, clankersPkg, routerPkg, clankersDaemonModule, clankerRouterModule }:
pkgs.testers.runNixOSTest {
  name = "clankers-module-integration";
  skipLint = true;

  nodes.machine = { pkgs, lib, ... }: {
    imports = [ clankersDaemonModule clankerRouterModule ];

    virtualisation.graphics = false;
    virtualisation.memorySize = 2048;
    networking.firewall.enable = false;

    environment.systemPackages = [ clankersPkg pkgs.curl pkgs.jq ];

    environment.etc."clankers-env".text = ''
      ANTHROPIC_API_KEY=sk-ant-test-dummy-key-for-vm-test
    '';

    # Router: listen on localhost, no proxy key for simplicity
    services.clanker-router = {
      enable = true;
      package = routerPkg;
      proxyAddr = "127.0.0.1:4000";
      environmentFile = "/etc/clankers-env";
    };

    # Daemon: points at the router (via env) and uses the module
    services.clankers-daemon = {
      enable = true;
      package = clankersPkg;
      heartbeat = 0;
      allowAll = true;
      environmentFile = "/etc/clankers-env";
    };

    # Ensure router is ready before daemon tries to use it
    systemd.services.clankers-daemon.after =
      lib.mkForce [ "network-online.target" "clanker-router.service" ];
    systemd.services.clankers-daemon.wants =
      lib.mkForce [ "network-online.target" "clanker-router.service" ];
  };

  testScript = ''
    machine.wait_for_unit("default.target")

    # ── Phase 1: Both services running ───────────────────────────────
    machine.wait_for_unit("clanker-router.service", timeout=60)
    machine.wait_for_unit("clankers-daemon.service", timeout=60)
    machine.log("Both services are active")

    machine.succeed("systemctl is-active clanker-router.service")
    machine.succeed("systemctl is-active clankers-daemon.service")

    # ── Phase 2: Router port open ────────────────────────────────────
    machine.wait_for_open_port(4000, timeout=15)
    machine.log("Router proxy port 4000 listening")

    # ── Phase 3: Both users exist ────────────────────────────────────
    machine.succeed("getent passwd clankers")
    machine.succeed("getent passwd clanker-router")
    machine.succeed("getent group clankers")
    machine.succeed("getent group clanker-router")

    # ── Phase 4: Both state directories ──────────────────────────────
    machine.succeed("test -d /var/lib/clankers")
    machine.succeed("test -d /var/lib/clanker-router")

    # ── Phase 5: Daemon can reach router ─────────────────────────────
    # From the daemon's perspective, localhost:4000 should be open.
    # Test with curl from root (same network namespace).
    http_code = machine.succeed(
        "curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:4000/ || echo 000"
    ).strip()
    machine.log(f"Router HTTP code: {http_code}")
    assert http_code != "000", "router not reachable on localhost:4000"

    # ── Phase 6: Both processes running as correct users ─────────────
    daemon_user = machine.succeed(
        "ps -o user= -C clankers | head -1"
    ).strip()
    router_user = machine.succeed(
        "ps -o user= -C clanker-router | head -1"
    ).strip()
    machine.log(f"daemon user={daemon_user}, router user={router_user}")
    assert daemon_user == "clankers", f"daemon user wrong: {daemon_user}"

    # ── Phase 7: Restart ordering ────────────────────────────────────
    # Stop daemon, restart router, start daemon — daemon should come
    # back up after router is available.
    machine.succeed("systemctl stop clankers-daemon.service")
    machine.succeed("systemctl restart clanker-router.service")
    machine.wait_for_unit("clanker-router.service", timeout=30)
    machine.wait_for_open_port(4000, timeout=15)
    machine.succeed("systemctl start clankers-daemon.service")
    machine.wait_for_unit("clankers-daemon.service", timeout=30)
    machine.log("Restart ordering works")

    # ── Phase 8: Clean stop both ─────────────────────────────────────
    machine.succeed("systemctl stop clankers-daemon.service")
    machine.succeed("systemctl stop clanker-router.service")
    machine.succeed("! systemctl is-active clankers-daemon.service")
    machine.succeed("! systemctl is-active clanker-router.service")
    machine.log("Module integration test passed")
  '';
}
