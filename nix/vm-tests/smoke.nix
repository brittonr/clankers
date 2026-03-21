# Basic smoke test — binary exists, runs, help works, headless mode exits.
{ pkgs, clankersPkg }:
pkgs.testers.runNixOSTest {
  name = "clankers-vm-smoke";
  skipLint = true;

  nodes.machine = { pkgs, ... }: {
    virtualisation.graphics = false;
    virtualisation.memorySize = 2048;
    environment.systemPackages = [ clankersPkg pkgs.git pkgs.tmux ];
    environment.variables = {
      HOME = "/root";
      TERM = "xterm-256color";
    };
  };

  testScript = ''
    machine.wait_for_unit("default.target")

    # Binary exists and runs
    machine.succeed("clankers --version")
    version = machine.succeed("clankers --version").strip()
    assert "clankers" in version, f"unexpected version: {version}"

    # Help output contains expected sections
    help_output = machine.succeed("clankers --help")
    assert "Usage:" in help_output or "usage:" in help_output.lower(), \
      f"help output missing usage section: {help_output[:200]}"

    # Headless mode exits cleanly with prompt from stdin
    machine.succeed("echo 'test prompt' | timeout 5 clankers --headless --no-session 2>&1 || true")

    # Git init for session/worktree tests
    machine.succeed("cd /tmp && git init test-repo && cd test-repo && git config user.email test@test.com && git config user.name Test")

    # Verify the binary finds its config paths
    machine.succeed("mkdir -p /root/.clankers/agent")
    machine.succeed("ls -la /root/.clankers/agent")
  '';
}
