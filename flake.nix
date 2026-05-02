{
  description = "clankers — Rust terminal coding agent";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    unit2nix = {
      url = "github:brittonr/unit2nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";

    # rat-* TUI crates used by clankers-tui. The workspace Cargo.toml
    # references these as path deps (../subwayrat/...); we pin them here
    # and patch the source so they resolve inside the Nix sandbox.
    subwayrat-src = {
      url = "git+ssh://git@github.com/brittonr/subwayrat.git";
      flake = false;
    };

    # subwayrat itself depends on ratcore via ../ratcore.
    ratcore-src = {
      url = "github:brittonr/ratcore";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, unit2nix, rust-overlay, flake-utils, subwayrat-src, ratcore-src, ... }:
    {
      nixosModules = {
        clankers-daemon = import ./nix/modules/clankers-daemon.nix;
        clanker-router = import ./nix/modules/clanker-router.nix;
        default = { imports = [
          self.nixosModules.clankers-daemon
          self.nixosModules.clanker-router
        ]; };
      };
    }
    //
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          localSystem = system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        isX86Linux = system == "x86_64-linux";
        liveChecksEnabled = builtins.getEnv "CLANKERS_ENABLE_LIVE_CHECKS" == "1";
        liveRatsDir = builtins.getEnv "CLANKERS_LIVE_RATS_DIR";
        liveRepoDir = builtins.getEnv "PWD";
        liveRatsSrc = builtins.path {
          path = if liveRatsDir != "" then liveRatsDir else liveRepoDir + "/../rats";
          name = "clankers-live-rats-src";
        };

        # ── Main workspace (unit2nix auto mode) ─────────────────────────
        ws = unit2nix.lib.${system}.buildFromUnitGraphAuto {
          inherit pkgs rustToolchain;
          src = ./.;
          workspace = true;
          noLocked = true;
          clippyArgs = [ "-D" "warnings" ];
          # rat-* TUI crates live in a sibling repo and subwayrat depends on
          # ratcore as another sibling path dependency.
          externalSources = {
            "../subwayrat" = subwayrat-src;
            "../ratcore" = ratcore-src;
          };
          buildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
            rustc = rustToolchain;
          };
          extraCrateOverrides = {
            aws-lc-rs = attrs: {
              nativeBuildInputs = [ pkgs.cmake pkgs.go ];
            };
            # libmimalloc-sys vendors mimalloc and builds it via cc/build.rs.
            # No extra native inputs needed; keep explicit override so
            # unit2nix knows this links crate was reviewed.
            libmimalloc-sys = attrs: {};
            ort-sys = attrs: {
              nativeBuildInputs = [ pkgs.pkg-config ];
              buildInputs = [ pkgs.onnxruntime pkgs.onnxruntime.dev ];
              ORT_STRATEGY = "system";
              ORT_LIB_LOCATION = "${pkgs.onnxruntime}";
            };
          };
        };

        # ── clanker-router standalone CLI binary ────────────────────────
        routerBuild = unit2nix.lib.${system}.buildFromUnitGraphAuto {
          inherit pkgs rustToolchain;
          src = ./crates/clanker-router;
          features = "cli";
          buildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
            rustc = rustToolchain;
          };
          extraCrateOverrides = {
            aws-lc-rs = attrs: {
              nativeBuildInputs = [ pkgs.cmake pkgs.go ];
            };
            libmimalloc-sys = attrs: {};
          };
        };

        # ── Additional derivations ──────────────────────────────────────
        clankersPkg = ws.workspaceMembers."clankers".build;
        verus = import ./nix/verus.nix { inherit pkgs; };
        clankers-docs = import ./nix/docs.nix { inherit pkgs; src = ./.; };
        clankers-plugins = import ./nix/plugins.nix {
          inherit pkgs rustToolchain unit2nix system;
          src = ./.;
        };
        routerPkg = routerBuild.rootCrate.build;

        # Shim for cargo-dylint in nix environments (no real rustup).
        # Dylint calls `rustup show active-toolchain` and `rustup which rustc`
        # to detect the compiler; this shim answers from the nix toolchain.
        rustup-shim = pkgs.writeShellScriptBin "rustup" ''
          case "$1 $2" in
            "show active-toolchain")
              echo "nightly-x86_64-unknown-linux-gnu (from nix)"
              ;;
            "which rustc")
              echo "$(rustc --print sysroot)/bin/rustc"
              ;;
            *)
              echo "rustup shim: unsupported command: $*" >&2
              exit 1
              ;;
          esac
        '';

        # The e2e harness invokes `cargo run -- ...` so local development keeps
        # exercising the same path. In Nix checks, run the already-built package
        # instead of rebuilding/fetching through Cargo inside the sandbox.
        cargo-run-clankers-shim = pkgs.writeShellScriptBin "cargo" ''
          if [ "''${1:-}" = "run" ]; then
            shift
            while [ "$#" -gt 0 ]; do
              case "$1" in
                --)
                  shift
                  break
                  ;;
                *)
                  shift
                  ;;
              esac
            done
            exec ${clankersPkg}/bin/clankers "$@"
          fi

          exec ${rustToolchain}/bin/cargo "$@"
        '';
      in
      {
        packages = {
          default = clankersPkg;
          clankers = clankersPkg;
          clanker-router = routerBuild.rootCrate.build;
          all = ws.allWorkspaceMembers;
          docs = clankers-docs;
          inherit clankers-plugins;
        } // pkgs.lib.optionalAttrs isX86Linux {
          inherit verus;
        };

        checks = {
          # Per-crate test runners (unit2nix --workspace).
          # Root `clankers` crate excluded — its integration tests need
          # CARGO_BIN_EXE_clankers which isn't available in buildRustCrate.
          inherit (ws.test.check)
            clanker-auth
            clanker-message
            clanker-plugin-sdk
            clanker-router
            clanker-tui-types
            clankers-agent-defs
            clankers-controller
            clankers-db
            clankers-engine
            clankers-engine-host
            clankers-matrix
            clankers-model-selection
            clankers-procmon
            clankers-protocol
            clankers-tool-host
            clankers-tui
            clankers-zellij
            ;

          clippy = ws.clippy.allWorkspaceMembers;

          fmt = pkgs.runCommand "cargo-fmt-check" {
            nativeBuildInputs = [ rustToolchain ];
            src = ./.;
          } ''
            cd $src
            cargo fmt --check
            touch $out
          '';

          docs = clankers-docs;

          e2e-fake = pkgs.runCommand "clankers-e2e-fake" {
            nativeBuildInputs = [
              cargo-run-clankers-shim
              pkgs.bash
              pkgs.coreutils
              pkgs.findutils
              pkgs.gnugrep
              pkgs.python3
            ];
          } ''
            export HOME="$TMPDIR/home"
            export XDG_CONFIG_HOME="$TMPDIR/xdg-config"
            export XDG_CACHE_HOME="$TMPDIR/xdg-cache"
            export XDG_DATA_HOME="$TMPDIR/xdg-data"
            export XDG_RUNTIME_DIR="$TMPDIR/run"
            export CLANKERS_NO_DAEMON=1
            export CLANKERS_FAKE_PROVIDER=1
            export CARGO_TARGET_DIR="$TMPDIR/cargo-target"
            export CLANKERS_TEST_RESULT_DIR="$TMPDIR/test-harness"

            mkdir -p \
              "$HOME" \
              "$XDG_CONFIG_HOME" \
              "$XDG_CACHE_HOME" \
              "$XDG_DATA_HOME" \
              "$XDG_RUNTIME_DIR" \
              "$CARGO_TARGET_DIR"

            cd ${./.}
            ./scripts/test-harness.sh e2e fake

            touch "$out"
          '';

          plugin-wasm-fresh = pkgs.runCommand "plugin-wasm-fresh" {
            nativeBuildInputs = [ pkgs.diffutils ];
          } ''
            # Verify committed .wasm files match what nix builds from source.
            # Fails if someone edits plugin Rust code without rebuilding WASM.
            for plugin_dir in ${clankers-plugins}/lib/clankers/plugins/*/; do
              name=$(basename "$plugin_dir")
              nix_wasm="$plugin_dir/$name.wasm"
              repo_wasm="${./.}/plugins/$name/$name.wasm"
              if [ ! -f "$repo_wasm" ]; then
                continue  # plugin only exists in nix build, not committed
              fi
              if ! cmp -s "$nix_wasm" "$repo_wasm"; then
                echo "STALE: plugins/$name/$name.wasm differs from nix build"
                echo "  Run: nix build .#clankers-plugins && cp result/lib/clankers/plugins/$name/$name.wasm plugins/$name/"
                exit 1
              fi
            done
            echo "All committed plugin WASM files match nix build."
            touch $out
          '';

          tracey-coverage = pkgs.runCommand "tracey-coverage" {
            nativeBuildInputs = [ pkgs.tracey ];
            src = ./.;
          } ''
            cd $src
            tracey query status

            uncovered=$(tracey query uncovered 2>&1)
            if ! echo "$uncovered" | grep -q "0 uncovered"; then
              echo "ERROR: uncovered requirements found"
              echo "$uncovered"
              exit 1
            fi

            untested=$(tracey query untested 2>&1)
            if ! echo "$untested" | grep -q "0 untested"; then
              echo "ERROR: untested implementations found"
              echo "$untested"
              exit 1
            fi

            touch $out
          '';
        }
        // pkgs.lib.optionalAttrs liveChecksEnabled {
          live-aspen2-qwen36 = pkgs.runCommandLocal "clankers-live-aspen2-qwen36" {
            nativeBuildInputs = [
              rustToolchain
              pkgs.bash
              pkgs.cacert
              pkgs.coreutils
              pkgs.cargo-nextest
              pkgs.clang
              pkgs.findutils
              pkgs.git
              pkgs.gnugrep
              pkgs.mold
              pkgs.openssh
              pkgs.pkg-config
              pkgs.python3
            ];
            buildInputs = [
              pkgs.openssl
              pkgs.sqlite
              pkgs.libgit2
              pkgs.libssh2
              pkgs.zlib
              pkgs.zstd
            ];
            # This check is intentionally opt-in and local/live. It may need
            # evaluator impurity plus unsandboxed network access to contact
            # aspen2; the test itself still self-skips when unavailable.
            __noChroot = true;
            src = ./.;
          } ''
            export HOME="$TMPDIR/home"
            export XDG_CONFIG_HOME="$TMPDIR/xdg-config"
            export XDG_CACHE_HOME="$TMPDIR/xdg-cache"
            export XDG_DATA_HOME="$TMPDIR/xdg-data"
            export XDG_RUNTIME_DIR="$TMPDIR/run"
            export CLANKERS_NO_DAEMON=1
            export SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt
            export GIT_SSL_CAINFO=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt
            export CARGO_TARGET_DIR="$TMPDIR/cargo-target"
            export CLANKERS_TEST_RESULT_DIR="$TMPDIR/test-harness-live"

            mkdir -p \
              "$HOME" \
              "$XDG_CONFIG_HOME" \
              "$XDG_CACHE_HOME" \
              "$XDG_DATA_HOME" \
              "$XDG_RUNTIME_DIR" \
              "$CARGO_TARGET_DIR"

            cp -R $src "$TMPDIR/source"
            cp -R ${liveRatsSrc}/subwayrat "$TMPDIR/subwayrat"
            git clone https://github.com/brittonr/ratcore "$TMPDIR/ratcore"
            git -C "$TMPDIR/ratcore" checkout 16333a505696b324637f021b657c474600a9b838
            chmod -R u+w "$TMPDIR/source" "$TMPDIR/subwayrat" "$TMPDIR/ratcore"

            python3 - <<'PY'
            import os
            from pathlib import Path
            tmpdir = Path(os.environ["TMPDIR"])
            source = tmpdir / "source"
            subwayrat = tmpdir / "subwayrat"
            ratcore = tmpdir / "ratcore"

            cargo_toml = source / "Cargo.toml"
            text = cargo_toml.read_text()
            text = text.replace(
                '[patch."ssh://git@github.com/brittonr/ratcore.git"]\n'
                'ratcore = { git = "ssh://git@github.com:22/brittonr/ratcore.git", rev = "16333a505696b324637f021b657c474600a9b838" }',
                '[patch."ssh://git@github.com/brittonr/ratcore.git"]\n'
                f'ratcore = {{ path = "{ratcore}" }}\n\n'
                '[patch."ssh://git@github.com:22/brittonr/ratcore.git"]\n'
                f'ratcore = {{ path = "{ratcore}" }}',
            )
            text += f"""

            [patch.\"ssh://git@github.com/brittonr/subwayrat.git\"]
            rat-branches = {{ path = \"{subwayrat}/crates/rat-branches\" }}
            rat-keymap = {{ path = \"{subwayrat}/crates/rat-keymap\" }}
            rat-leaderkey = {{ path = \"{subwayrat}/crates/rat-leaderkey\" }}
            rat-inline = {{ path = \"{subwayrat}/crates/rat-inline\" }}
            rat-markdown = {{ path = \"{subwayrat}/crates/rat-markdown\" }}
            rat-spinner = {{ path = \"{subwayrat}/crates/rat-spinner\" }}
            rat-widgets = {{ path = \"{subwayrat}/crates/rat-widgets\" }}
            """
            cargo_toml.write_text(text)
            PY

            cd "$TMPDIR/source"
            ./scripts/test-harness.sh live aspen2-qwen36

            touch "$out"
          '';
        }
        // pkgs.lib.optionalAttrs isX86Linux {
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
          vm-smoke = import ./nix/vm-tests/smoke.nix { inherit pkgs clankersPkg; };
          vm-remote-daemon = import ./nix/vm-tests/remote-daemon.nix { inherit pkgs clankersPkg; };
          vm-session-recovery = import ./nix/vm-tests/session-recovery.nix { inherit pkgs clankersPkg; };
          vm-module-daemon = import ./nix/vm-tests/module-daemon.nix {
            inherit pkgs clankersPkg;
            clankersDaemonModule = self.nixosModules.clankers-daemon;
          };
          vm-module-router = import ./nix/vm-tests/module-router.nix {
            inherit pkgs routerPkg;
            clankerRouterModule = self.nixosModules.clanker-router;
          };
          vm-module-integration = import ./nix/vm-tests/module-integration.nix {
            inherit pkgs clankersPkg routerPkg;
            clankersDaemonModule = self.nixosModules.clankers-daemon;
            clankerRouterModule = self.nixosModules.clanker-router;
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
            pkgs.tmux
            pkgs.cargo-insta
            pkgs.mdbook
            pkgs.sunxi-tools
            pkgs.sd-mux-ctrl
            pkgs.usbutils
            pkgs.espeak-ng
            rustup-shim
          ] ++ pkgs.lib.optionals isX86Linux [
            verus
          ];

          shellHook = ''
            export PATH="$PWD/target/debug:$PATH"
            export LIBRARY_PATH="${pkgs.sqlite.out}/lib''${LIBRARY_PATH:+:$LIBRARY_PATH}"
          '';
        };
      }
    );
}
