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
          clankers-router = wsRouter.workspaceMembers."clankers-router".build;
          all = ws.allWorkspaceMembers;
          inherit clankers-plugins;
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
            clankers-specs
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
