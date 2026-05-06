# Plugin runtime smoke — boot a VM and exercise packaged Extism plus stdio plugins.
{ pkgs, clankersPkg, clankers-plugins, src }:
let
  restrictedProbePlugin = pkgs.runCommand "clankers-stdio-restricted-probe" { } ''
    mkdir -p "$out"
    cat > "$out/plugin.json" <<JSON
    {
      "name": "clankers-stdio-restricted-probe",
      "version": "0.1.0",
      "description": "VM-only restricted stdio sandbox probe.",
      "kind": "stdio",
      "permissions": [],
      "stdio": {
        "command": "${pkgs.python3}/bin/python3",
        "args": ["plugin.py"],
        "working_dir": "plugin-dir",
        "sandbox": "restricted",
        "writable_roots": ["allowed"],
        "allow_network": false
      }
    }
JSON
    cat > "$out/plugin.py" <<'PY'
    import json
    import sys

    PROTOCOL = 1
    PLUGIN = "clankers-stdio-restricted-probe"
    TOOL = "stdio_restricted_probe"

    def read_frame():
        length_bytes = sys.stdin.buffer.read(4)
        if not length_bytes:
            return None
        if len(length_bytes) != 4:
            raise EOFError("short frame length")
        length = int.from_bytes(length_bytes, "big")
        payload = sys.stdin.buffer.read(length)
        if len(payload) != length:
            raise EOFError("short frame payload")
        return json.loads(payload)

    def write_frame(frame):
        payload = json.dumps(frame, separators=(",", ":")).encode("utf-8")
        sys.stdout.buffer.write(len(payload).to_bytes(4, "big"))
        sys.stdout.buffer.write(payload)
        sys.stdout.buffer.flush()

    def startup():
        frame = read_frame()
        if frame is None or frame.get("type") != "hello":
            raise SystemExit("missing host hello")
        write_frame({"type": "hello", "plugin_protocol": PROTOCOL, "plugin": PLUGIN, "version": "0.1.0"})
        write_frame({"type": "ready", "plugin_protocol": PROTOCOL})
        write_frame({"type": "register_tools", "plugin_protocol": PROTOCOL, "tools": [{
            "name": TOOL,
            "description": "Probe restricted sandbox writable boundary.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "allowed_path": {"type": "string"},
                    "denied_path": {"type": "string"}
                },
                "required": ["allowed_path", "denied_path"],
                "additionalProperties": False
            }
        }]})

    def try_write(path):
        try:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write("probe")
            return True
        except Exception:
            return False

    def main():
        startup()
        while True:
            frame = read_frame()
            if frame is None:
                return
            if frame.get("type") == "shutdown":
                return
            if frame.get("type") != "tool_invoke":
                continue
            call_id = frame["call_id"]
            args = frame.get("args", {})
            allowed = try_write(args["allowed_path"])
            denied = try_write(args["denied_path"])
            write_frame({
                "type": "tool_result",
                "plugin_protocol": PROTOCOL,
                "call_id": call_id,
                "content": f"allowed={str(allowed).lower()};denied={str(denied).lower()}"
            })

    if __name__ == "__main__":
        main()
PY
    chmod +x "$out/plugin.py"
  '';
in
pkgs.testers.runNixOSTest {
  name = "clankers-vm-plugin-runtime";
  skipLint = true;

  nodes.machine = { pkgs, ... }: {
    virtualisation.graphics = false;
    virtualisation.memorySize = 3072;
    environment.systemPackages = [ clankersPkg pkgs.coreutils pkgs.gnugrep pkgs.python3 ];
    environment.variables = {
      HOME = "/root";
      TERM = "xterm-256color";
    };
  };

  testScript = ''
    import json
    import shlex

    machine.wait_for_unit("default.target")
    machine.succeed("clankers --version")

    machine.succeed("mkdir -p /root/plugin-vm/.clankers /root/plugin-vm/allowed /root/plugin-vm/denied-parent /run/clankers-vm")

    env = "HOME=/root XDG_CONFIG_HOME=/root/.config XDG_CACHE_HOME=/root/.cache XDG_DATA_HOME=/root/.local/share XDG_RUNTIME_DIR=/run/clankers-vm CLANKERS_NO_DAEMON=1 CLANKERS_STDIO_TOOL_TIMEOUT_MS=5000"

    def reset_plugins():
        machine.succeed("rm -rf /root/plugin-vm/plugins && mkdir -p /root/plugin-vm/plugins")
        machine.succeed("cp -R ${clankers-plugins}/lib/clankers/plugins/. /root/plugin-vm/plugins/")
        machine.succeed("cp -R ${src}/examples/plugins/clankers-stdio-echo /root/plugin-vm/plugins/clankers-stdio-echo")
        machine.succeed("cp -R ${restrictedProbePlugin} /root/plugin-vm/plugins/clankers-stdio-restricted-probe")
        machine.succeed("chmod -R u+w /root/plugin-vm/plugins/clankers-stdio-echo /root/plugin-vm/plugins/clankers-stdio-restricted-probe")
        machine.succeed("python3 - <<'PY'\nimport json\nfrom pathlib import Path\nfor name in ['clankers-stdio-echo']:\n    path = Path('/root/plugin-vm/plugins') / name / 'plugin.json'\n    data = json.loads(path.read_text())\n    data['stdio']['command'] = '${pkgs.python3}/bin/python3'\n    data['stdio']['args'] = ['plugin.py']\n    path.write_text(json.dumps(data, indent=2) + '\\n')\nPY")

    def run_clankers(command):
        return machine.succeed(f"cd /root/plugin-vm && {env} clankers --cwd /root/plugin-vm {command}")

    reset_plugins()
    listing = run_clankers("plugin list --verbose")
    assert "clankers-hash" in listing, listing
    assert "hash_text" in listing, listing
    assert "clankers-stdio-echo" in listing, listing
    assert "clankers-stdio-echo v0.1.0 [Error" not in listing, listing

    hash_output = run_clankers("plugin call clankers-hash hash_text '{\"text\":\"hello\",\"algorithm\":\"sha256\"}'")
    hash_result = json.loads(hash_output)
    assert hash_result["is_error"] is False, hash_result
    assert "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824" in json.dumps(hash_result), hash_result

    restricted_args = json.dumps({
        "allowed_path": "/root/plugin-vm/allowed/probe.txt",
        "denied_path": "/root/plugin-vm/denied-parent/probe.txt",
    })
    restricted_cmd = (
        "cd /root/plugin-vm && "
        + env
        + " clankers --cwd /root/plugin-vm plugin call clankers-stdio-restricted-probe stdio_restricted_probe "
        + shlex.quote(restricted_args)
        + " 2>&1"
    )
    restricted_output = machine.succeed(restricted_cmd + " || true")
    if "allowed=true;denied=false" in restricted_output:
        machine.succeed("test -f /root/plugin-vm/allowed/probe.txt")
        machine.succeed("test ! -e /root/plugin-vm/denied-parent/probe.txt")
    else:
        assert "did not become available" in restricted_output or "restricted sandbox" in restricted_output, restricted_output
        machine.succeed("test ! -e /root/plugin-vm/denied-parent/probe.txt")
  '';
}
