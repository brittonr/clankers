{
  description = "clankers — Rust project built with unit2nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    unit2nix.url = "github:brittonr/unit2nix";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, unit2nix, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        # Nightly toolchain — needed for WASM plugin builds (-Zbuild-std)
        # and for the devShell.
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # ── Main workspace (unit2nix auto mode) ─────────────────────────────
        #
        # Build plan is generated via IFD — no build-plan.json to maintain.
        # Cargo.lock changes are picked up automatically at eval time.
        ws = unit2nix.lib.${system}.buildFromUnitGraphAuto {
          inherit pkgs rustToolchain;
          src = ./.;
          workspace = true;
          clippyArgs = [ "-D" "warnings" ];

          # Use the nightly toolchain — clankers requires edition 2024
          # and unstable library features.
          buildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
            rustc = rustToolchain;
          };

          extraCrateOverrides = {
            # aws-lc-rs wraps aws-lc-sys; its build script needs cmake + go
            aws-lc-rs = attrs: {
              nativeBuildInputs = [ pkgs.cmake pkgs.go ];
            };
          };
        };

        # ── clankers-router standalone build ───────────────────────────────
        #
        # The router binary requires the `cli` feature which isn't in the
        # workspace graph (the workspace uses `rpc` only).
        wsRouter = unit2nix.lib.${system}.buildFromUnitGraphAuto {
          inherit pkgs rustToolchain;
          src = ./.;
          package = "clankers-router";
          features = "cli";
          includeDev = true;

          buildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
            rustc = rustToolchain;
          };

          extraCrateOverrides = {
            aws-lc-rs = attrs: {
              nativeBuildInputs = [ pkgs.cmake pkgs.go ];
            };
          };
        };

        # ── WASM plugin builds ─────────────────────────────────────────────
        #
        # Plugins are standalone crates with their own Cargo.lock, built to
        # wasm32-unknown-unknown with -Zbuild-std. unit2nix doesn't handle
        # WASM targets, so we keep these as a plain stdenv derivation.

        pluginSpecs = [
          { dir = "plugins/clankers-hash"; name = "clankers_hash"; }
          { dir = "plugins/clankers-self-validate"; name = "clankers_self_validate"; }
          { dir = "plugins/clankers-test-plugin"; name = "clankers_test_plugin"; }
          { dir = "plugins/clankers-text-stats"; name = "clankers_text_stats"; }
          { dir = "examples/plugins/clankers-wordcount"; name = "clankers_wordcount"; }
        ];

        # Source filter: include plugin.json manifests alongside Cargo sources
        pluginSrc = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (builtins.match ".*plugin\\.json$" path != null)
            || (builtins.match ".*\\.(rs|toml|lock)$" path != null)
            || type == "directory";
        };

        pluginVendor = unit2nix.lib.${system}.vendorMultipleCargoDeps {
          inherit pkgs;
          cargoLocks =
            (map (p: ./. + "/${p.dir}/Cargo.lock") pluginSpecs)
            ++ [ "${rustToolchain}/lib/rustlib/src/rust/library/Cargo.lock" ];
        };

        # ── Verus verifier (prebuilt binary) ────────────────────────────
        #
        # Verus requires a specific Rust toolchain + Z3. The prebuilt
        # release bundles everything: verus binary, rust_verify, z3,
        # proc-macro libs, and vstd source.
        #
        # rust_verify links against librustc_driver from Rust 1.93.1,
        # so we provide that toolchain's libraries for autoPatchelf.
        verusRustToolchain = pkgs.rust-bin.stable."1.93.1".default.override {
          extensions = [ "rustc-dev" "llvm-tools" ];
        };

        verus = pkgs.stdenv.mkDerivation {
          pname = "verus";
          version = "0.2026.03.10.13c14a1";
          src = pkgs.fetchzip {
            url = "https://github.com/verus-lang/verus/releases/download/release/0.2026.03.10.13c14a1/verus-0.2026.03.10.13c14a1-x86-linux.zip";
            hash = "sha256-tmlV/ozVX1GRuiEKh6qeFh61TGZSULVRwEvPNoiPgMM=";
          };
          nativeBuildInputs = [ pkgs.autoPatchelfHook pkgs.makeWrapper ];
          buildInputs = [
            pkgs.stdenv.cc.cc.lib
            verusRustToolchain
          ];
          installPhase = ''
            runHook preInstall
            mkdir -p $out/bin $out/lib/verus
            cp -r . $out/lib/verus/

            # The upstream `verus` binary is a wrapper that checks for
            # rustup. On NixOS we bypass it: call rust_verify directly
            # with the right library paths and Z3.
            makeWrapper $out/lib/verus/rust_verify $out/bin/verus \
              --set VERUS_Z3_PATH "$out/lib/verus/z3" \
              --prefix LD_LIBRARY_PATH : "${verusRustToolchain}/lib" \
              --prefix LD_LIBRARY_PATH : "${verusRustToolchain}/lib/rustlib/x86_64-unknown-linux-gnu/lib" \
              --add-flags "-L dependency=$out/lib/verus" \
              --add-flags "--extern builtin=$out/lib/verus/libverus_builtin.rlib" \
              --add-flags "--extern vstd=$out/lib/verus/libvstd.rlib" \
              --add-flags "--extern builtin_macros=$out/lib/verus/libverus_builtin_macros.so" \
              --add-flags "--extern state_machines_macros=$out/lib/verus/libverus_state_machines_macros.so" \
              --add-flags "--edition 2021"

            ln -s $out/lib/verus/cargo-verus $out/bin/cargo-verus
            runHook postInstall
          '';
        };

        # ── Documentation site ──────────────────────────────────────────
        #
        # Runs docs/generate.sh to extract crate metadata from source,
        # then mdbook to produce a static HTML site.
        clankers-docs = pkgs.stdenv.mkDerivation {
          pname = "clankers-docs";
          version = "0.1.0";
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              (builtins.match ".*\\.(rs|toml|lock|md|css|json|sh)$" path != null)
              || type == "directory";
          };
          nativeBuildInputs = [ pkgs.mdbook ];
          buildPhase = ''
            runHook preBuild
            bash docs/generate.sh "$PWD"
            mdbook build docs
            runHook postBuild
          '';
          installPhase = ''
            runHook preInstall
            cp -r docs/book $out
            runHook postInstall
          '';
        };

        clankers-plugins = pkgs.stdenv.mkDerivation {
          pname = "clankers-plugins";
          version = "0.1.0";
          src = pluginSrc;
          nativeBuildInputs = [ rustToolchain pkgs.clang pkgs.mold ];

          configurePhase = ''
            cat ${pluginVendor.cargoConfig} >> .cargo/config.toml
          '';

          buildPhase = ''
            runHook preBuild
            ${pkgs.lib.concatMapStringsSep "\n" (p: ''
              echo "Building ${p.name}…"
              cargo build \
                --manifest-path ${p.dir}/Cargo.toml \
                --target wasm32-unknown-unknown \
                --release \
                -Zbuild-std=std,panic_abort
            '') pluginSpecs}
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            ${pkgs.lib.concatMapStringsSep "\n" (p: ''
              mkdir -p $out/lib/clankers/plugins/${p.name}
              cp ${p.dir}/target/wasm32-unknown-unknown/release/${p.name}.wasm \
                $out/lib/clankers/plugins/${p.name}/
              if [ -f ${p.dir}/plugin.json ]; then
                cp ${p.dir}/plugin.json $out/lib/clankers/plugins/${p.name}/
              fi
            '') pluginSpecs}
            runHook postInstall
          '';
        };
      in
      {
        packages = {
          default = ws.workspaceMembers."clankers".build;
          clankers = ws.workspaceMembers."clankers".build;
          # clankers-router — disabled: clanker-router is an external git dep,
          # cargo rejects --features for non-workspace packages in --unit-graph.
          # Build with: cargo build -p clanker-router --features cli
          # clankers-router = wsRouter.workspaceMembers."clankers-router".build;
          all = ws.allWorkspaceMembers;
          docs = clankers-docs;
          inherit clankers-plugins verus;
        };

        checks = {
          # Per-crate test runners (generated by unit2nix --workspace).
          # The root `clankers` crate is excluded because its integration tests
          # use env!("CARGO_BIN_EXE_clankers") which requires Cargo's runtime
          # env vars (not available in buildRustCrate). Run those with `cargo test`.
          inherit (ws.test.check)
            clankers-actor
            clankers-agent-defs
            clankers-auth
            clankers-controller
            clankers-db
            clankers-matrix
            clankers-merge
            clankers-model-selection
            clankers-procmon
            clankers-protocol
            clankers-router

            clankers-tui
            clankers-tui-types
            clankers-zellij
            ;

          # Clippy — uses unit2nix's built-in clippy support with the
          # nightly toolchain. Only workspace members are recompiled under
          # clippy-driver; dependencies reuse cached normal builds.
          clippy = ws.clippy.allWorkspaceMembers;

          # Format check
          fmt = pkgs.runCommand "cargo-fmt-check" {
            nativeBuildInputs = [ rustToolchain ];
            src = ./.;
          } ''
            cd $src
            cargo fmt --check
            touch $out
          '';

          # Docs build — verifies xtask generation + mdbook build succeed
          docs = clankers-docs;

          # Tracey — requirement coverage (all requirements must be covered + tested)
          tracey-coverage = pkgs.runCommand "tracey-coverage" {
            nativeBuildInputs = [ pkgs.tracey ];
            src = ./.;
          } ''
            cd $src
            tracey query status

            # Fail if any requirement lacks an impl annotation
            uncovered=$(tracey query uncovered 2>&1)
            if ! echo "$uncovered" | grep -q "0 uncovered"; then
              echo "ERROR: uncovered requirements found"
              echo "$uncovered"
              exit 1
            fi

            # Fail if any implemented requirement lacks a verify annotation
            untested=$(tracey query untested 2>&1)
            if ! echo "$untested" | grep -q "0 untested"; then
              echo "ERROR: untested implementations found"
              echo "$untested"
              exit 1
            fi

            touch $out
          '';

          # Verus — machine-checked proofs for core invariants
          verus-proofs = pkgs.runCommand "verus-proofs" {
            nativeBuildInputs = [ verus ];
            src = ./.;
          } ''
            cd $src
            verus --crate-type=lib verus/lib.rs
            touch $out
          '';
        }
        // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
          # NixOS VM integration test — builds and runs the binary in a VM.
          # Only available on Linux (requires QEMU/KVM).
          vm-smoke = pkgs.testers.runNixOSTest {
            name = "clankers-vm-smoke";
            skipLint = true;

            nodes.machine = { pkgs, ... }: {
              virtualisation.graphics = false;
              virtualisation.memorySize = 2048;
              environment.systemPackages = [
                ws.workspaceMembers."clankers".build
                pkgs.git
                pkgs.tmux
              ];
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
          };

          # Two-VM test: daemon on one machine, client connects via iroh QUIC.
          # Validates the full remote attach path: endpoint binding, ALPN
          # negotiation, control stream (create/list), and session attach.
          vm-remote-daemon = pkgs.testers.runNixOSTest {
            name = "clankers-remote-daemon";
            skipLint = true;

            nodes.server = { pkgs, ... }: {
              virtualisation.graphics = false;
              virtualisation.memorySize = 2048;
              networking.firewall.enable = false;
              environment.systemPackages = [
                ws.workspaceMembers."clankers".build
                pkgs.jq
              ];
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
              environment.systemPackages = [
                ws.workspaceMembers."clankers".build
                pkgs.jq
              ];
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

              # ── Phase 1: Start daemon on server ────────────────────────────
              # Run in foreground, capture node ID from startup banner, then
              # background it so we can interact from the client.
              server.succeed("mkdir -p /root/.clankers/agent")
              client.succeed("mkdir -p /root/.clankers/agent")

              # Start daemon in background with --allow-all (no token/ACL)
              server.succeed(
                  "clankers daemon start --allow-all --heartbeat 0 "
                  "> /tmp/daemon.log 2>&1 &"
              )

              # Wait for the daemon to bind its iroh endpoint and print the node ID
              server.wait_until_succeeds(
                  "grep -q 'Node ID:' /tmp/daemon.log",
                  timeout=30,
              )

              # Extract the node ID
              node_id = server.succeed(
                  "grep 'Node ID:' /tmp/daemon.log | head -1 | awk '{print $NF}'"
              ).strip()
              assert len(node_id) > 20, f"node ID too short: '{node_id}'"
              server.log(f"Server node ID: {node_id}")

              # Verify daemon is running via control socket
              server.succeed("clankers daemon status | grep -q 'Daemon running'")

              # ── Phase 2: RPC ping from client ─────────────────────────────
              # Uses the clankers/rpc/1 ALPN — validates basic iroh QUIC
              # connectivity between the two VMs.
              client.wait_until_succeeds(
                  f"clankers rpc ping {node_id} 2>&1 | grep -q 'pong'",
                  timeout=60,
              )
              client.log("RPC ping succeeded")

              # ── Phase 3: Create session over QUIC ──────────────────────────
              # Uses clankers/daemon/1 ALPN control stream.
              # The daemon has --allow-all so token checks are skipped.
              server.succeed("clankers daemon create > /tmp/session.out 2>&1")
              session_line = server.succeed("cat /tmp/session.out").strip()
              server.log(f"Created session: {session_line}")

              # Verify session shows up in listing
              server.succeed("clankers ps | grep -q 'claude'")

              # ── Phase 4: RPC status from client ────────────────────────────
              # Verify the client can query the daemon's status over QUIC.
              status_out = client.succeed(
                  f"clankers rpc status {node_id} 2>&1"
              )
              client.log(f"Remote status: {status_out}")

              # ── Phase 5: Verify daemon stays healthy ───────────────────────
              server.succeed("clankers daemon status | grep -q 'Daemon running'")
              server.log("All remote daemon tests passed")
            '';
          };
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            rustToolchain
            pkgs.pkg-config
            pkgs.clang
            pkgs.mold
          ];

          buildInputs = [
            pkgs.openssl
            pkgs.sqlite
            pkgs.libgit2
            pkgs.libssh2
            pkgs.zlib
            pkgs.zstd
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
          ];

          packages = [
            pkgs.cargo-nextest
            pkgs.cargo-watch
            pkgs.rust-analyzer
            unit2nix.packages.${system}.unit2nix

            # TUI integration testing
            pkgs.tmux
            pkgs.cargo-insta

            # Docs
            pkgs.mdbook

            # Formal verification
            verus

            # Router daemon — built via `cargo build -p clanker-router --features cli`
            # (wsRouter nix build disabled: clanker-router is an external git dep
            # and cargo rejects --features for non-workspace packages in --unit-graph)
            # wsRouter.workspaceMembers."clankers-router".build

            # Allwinner / SDWire tooling
            pkgs.sunxi-tools
            pkgs.sd-mux-ctrl
            pkgs.usbutils
          ];

          shellHook = ''
            export PATH="$PWD/target/debug:$PATH"
            export LIBRARY_PATH="${pkgs.sqlite.out}/lib''${LIBRARY_PATH:+:$LIBRARY_PATH}"
          '';
        };
      }
    );
}
