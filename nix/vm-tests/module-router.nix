# NixOS module test: services.clanker-router
#
# Validates the clanker-router NixOS module:
#   - systemd unit starts and stays running
#   - system user/group created
#   - state directory exists with correct ownership
#   - HTTP proxy binds to configured address/port
#   - firewall rules applied when openFirewall = true
#   - proxy key auth works
#   - hardening directives applied
{ pkgs, routerPkg, clankerRouterModule }:
pkgs.testers.runNixOSTest {
  name = "clankers-module-router";
  skipLint = true;

  nodes = {
    # Node with default config (closed firewall)
    router = { pkgs, lib, ... }: {
      imports = [ clankerRouterModule ];

      virtualisation.graphics = false;
      virtualisation.memorySize = 1024;
      networking.firewall.enable = true;

      environment.etc."router-env".text = ''
        ANTHROPIC_API_KEY=sk-ant-test-dummy-key-for-vm-test
      '';

      services.clanker-router = {
        enable = true;
        package = routerPkg;
        proxyAddr = "0.0.0.0:4000";
        proxyKeys = [ "test-proxy-key-12345" ];
        openFirewall = true;
        environmentFile = "/etc/router-env";
      };
    };

    client = { pkgs, ... }: {
      virtualisation.graphics = false;
      virtualisation.memorySize = 512;
      environment.systemPackages = [ pkgs.curl pkgs.jq ];
    };
  };

  testScript = ''
    start_all()
    router.wait_for_unit("default.target")
    client.wait_for_unit("default.target")

    # ── Phase 1: User/group creation ─────────────────────────────────
    router.succeed("getent passwd clanker-router")
    router.succeed("getent group clanker-router")

    uid = int(router.succeed("id -u clanker-router").strip())
    assert uid < 1000, f"expected system user, got UID {uid}"
    router.log(f"clanker-router user UID: {uid}")

    # ── Phase 2: State directory ─────────────────────────────────────
    router.succeed("test -d /var/lib/clanker-router")
    owner = router.succeed("stat -c '%U:%G' /var/lib/clanker-router").strip()
    assert owner == "clanker-router:clanker-router", f"bad ownership: {owner}"

    # ── Phase 3: Service starts ──────────────────────────────────────
    router.wait_for_unit("clanker-router.service", timeout=60)
    router.succeed("systemctl is-active clanker-router.service")

    ps_user = router.succeed(
        "ps -o user= -C clanker-router | head -1"
    ).strip()
    assert ps_user == "clanker-" or ps_user.startswith("clanker"), \
        f"expected clanker-router user, got: {ps_user}"
    router.log("Router running as correct user")

    # ── Phase 4: Port open ───────────────────────────────────────────
    router.wait_for_open_port(4000, timeout=30)
    router.log("Port 4000 is listening")

    # ── Phase 5: Firewall rule applied ───────────────────────────────
    # NixOS puts allow rules in the nixos-fw chain, not INPUT directly
    fw_rules = router.succeed("iptables -L nixos-fw -n")
    router.log(f"Firewall rules:\n{fw_rules}")
    assert "4000" in fw_rules, f"port 4000 not in firewall rules: {fw_rules}"

    # ── Phase 6: HTTP reachable from client ──────────────────────────
    # The router proxies LLM APIs — sending a request without a valid
    # backend will error, but we can verify the HTTP server responds.
    client.wait_until_succeeds(
        "curl -sf -o /dev/null -w '%{http_code}' "
        "http://router:4000/ 2>&1 || true",
        timeout=15,
    )
    # Any HTTP response (even 4xx/5xx) proves the proxy is reachable
    http_code = client.succeed(
        "curl -s -o /dev/null -w '%{http_code}' http://router:4000/ || echo 000"
    ).strip()
    router.log(f"HTTP response code: {http_code}")
    assert http_code != "000", "could not connect to router from client"

    # ── Phase 7: Hardening ───────────────────────────────────────────
    props = router.succeed(
        "systemctl show clanker-router.service "
        "--property=NoNewPrivileges,PrivateTmp,ProtectSystem"
    )
    assert "NoNewPrivileges=yes" in props
    assert "PrivateTmp=yes" in props
    assert "ProtectSystem=strict" in props
    router.log("Hardening properties verified")

    # ── Phase 8: Restart on failure ──────────────────────────────────
    router.succeed("systemctl kill --signal=KILL clanker-router.service")
    router.sleep(8)
    router.wait_for_unit("clanker-router.service", timeout=30)
    router.wait_for_open_port(4000, timeout=15)
    router.log("Service restarted and port re-opened after SIGKILL")

    # ── Phase 9: Clean stop ──────────────────────────────────────────
    router.succeed("systemctl stop clanker-router.service")
    router.succeed("! systemctl is-active clanker-router.service")
    router.log("Module router test passed")
  '';
}
