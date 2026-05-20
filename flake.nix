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

    ucan-src = {
      url = "git+ssh://git@github.com/OnixResearch/ucan.git";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, unit2nix, rust-overlay, flake-utils, subwayrat-src, ratcore-src, ucan-src, ... }:
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
            "../ucan" = ucan-src;
          };
          buildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
            cargo = rustToolchain;
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

        # Clippy uses unit2nix's clippy wrapper. Build it with the default
        # nixpkgs Rust toolchain so dependency rlibs and clippy-driver come
        # from the same compiler; rustToolchain is still supplied for the
        # unit-graph IFD step, which requires nightly cargo.
        clippyWs = unit2nix.lib.${system}.buildFromUnitGraphAuto {
          inherit pkgs rustToolchain;
          src = ./.;
          workspace = true;
          noLocked = true;
          clippyArgs = [ "-D" "warnings" ];
          externalSources = {
            "../subwayrat" = subwayrat-src;
            "../ratcore" = ratcore-src;
            "../ucan" = ucan-src;
          };
          extraCrateOverrides = {
            aws-lc-rs = attrs: {
              nativeBuildInputs = [ pkgs.cmake pkgs.go ];
            };
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
          src = ./.;
          package = "clanker-router";
          features = "cli";
          noLocked = true;
          externalSources = {
            "../subwayrat" = subwayrat-src;
            "../ratcore" = ratcore-src;
            "../ucan" = ucan-src;
          };
          buildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
            cargo = rustToolchain;
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
            clankers-config
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

          clippy = clippyWs.clippy.allWorkspaceMembers;

          fmt = pkgs.runCommand "cargo-fmt-check" {
            nativeBuildInputs = [ rustToolchain ];
            src = ./.;
          } ''
            cp -R $src source
            chmod -R u+w source
            mkdir -p ucan/src
            cat > ucan/Cargo.toml <<'EOF'
            [package]
            name = "ucan"
            version = "0.1.0"
            edition = "2024"
            [lib]
            path = "src/lib.rs"
            EOF
            touch ucan/src/lib.rs
            cd source
            cargo fmt --check
            touch $out
          '';

          docs = clankers-docs;

          embedded-sdk-release-receipt = pkgs.runCommand "embedded-sdk-release-receipt" {
            nativeBuildInputs = [ rustToolchain ];
            src = ./.;
          } ''
            cp -R $src source
            chmod -R u+w source
            cd source
            rustc --edition=2024 scripts/check-embedded-sdk-ci-wiring.rs -o check-embedded-sdk-ci-wiring
            ./check-embedded-sdk-ci-wiring
            touch $out
          '';

          openspec-review-gates = pkgs.runCommand "openspec-review-gates" {
            nativeBuildInputs = [ rustToolchain pkgs.clang pkgs.mold ];
            src = ./.;
          } ''
            cp -R $src source
            chmod -R u+w source
            cd source
            export HOME="$TMPDIR/home"
            export CARGO_HOME="$TMPDIR/cargo-home"
            mkdir -p "$HOME" "$CARGO_HOME"
            cargo -q -Zscript scripts/check-openspec-review-gates.rs
            touch $out
          '';

          e2e-fake = pkgs.runCommand "clankers-e2e-fake" {
            nativeBuildInputs = [
              pkgs.bash
              pkgs.coreutils
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
            export NO_COLOR=1
            export RUST_LOG=off

            mkdir -p \
              "$HOME" \
              "$XDG_CONFIG_HOME" \
              "$XDG_CACHE_HOME" \
              "$XDG_DATA_HOME" \
              "$XDG_RUNTIME_DIR" \
              "$TMPDIR/work/src/nested"

            cd "$TMPDIR/work"
            cat > Cargo.toml <<'EOF'
            [package]
            name = "clankers-readiness-fixture"
            EOF
            printf 'fn main() {}\n' > src/main.rs
            printf 'pub fn marker() {}\n' > src/nested/mod.rs

            run_clankers() {
              ${clankersPkg}/bin/clankers "$@"
            }

            run_clankers version | grep 'clankers 0.1.0'
            run_clankers --help | grep -E 'Usage:|Commands:'
            run_clankers config paths | grep 'Global config'
            run_clankers auth status 2>&1 | grep -E 'No authentication|Accounts:|API key|not authenticated'

            run_clankers -p 'Reply with exactly one word: yes' \
              | grep -i 'yes'
            run_clankers -p 'Use the bash tool to run: echo CLANKERS_TOOL_TEST_OK' \
              | grep 'CLANKERS_TOOL_TEST_OK'
            run_clankers -p 'Use the read tool to read the file Cargo.toml and tell me the package name' \
              | grep 'clankers'
            run_clankers -p "Use the find tool to find files named 'mod.rs' under src/" \
              | grep 'mod.rs'

            run_clankers --mode json -p 'Say hello' > json.out
            python3 - <<'PY'
            import json
            lines = [line for line in open('json.out', encoding='utf-8') if line.strip()]
            assert lines, 'json mode should emit at least one JSON line'
            for line in lines:
                json.loads(line)
            PY

            round_trip="$TMPDIR/clankers-e2e-write-test-nix"
            run_clankers -p "Use the write tool to create the file $round_trip with content 'hello world'."
            test -f "$round_trip"
            run_clankers -p "Use the edit tool to replace 'world' with 'clankers' in $round_trip."
            grep 'hello clankers' "$round_trip"
            run_clankers -p "Use the read tool to read $round_trip and show me the final content." \
              | grep 'clankers'

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

          tracey-coverage = if pkgs ? tracey then pkgs.runCommand "tracey-coverage" {
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
              echo "ERROR: untested requirements found"
              echo "$untested"
              exit 1
            fi

            touch $out
          '' else pkgs.runCommand "tracey-coverage-skipped" {
            src = ./.;
          } ''
            echo "tracey is not packaged in the pinned nixpkgs; skipping tracey coverage check"
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
          vm-plugin-runtime = import ./nix/vm-tests/plugin-runtime.nix {
            inherit pkgs clankersPkg clankers-plugins;
            src = ./.;
          };
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
          nixos-module-process-persistence =
            let
              fakeClankersPkg = pkgs.writeShellScriptBin "clankers" "exit 0";
              custom = nixpkgs.lib.nixosSystem {
                inherit system;
                modules = [
                  self.nixosModules.clankers-daemon
                  ({ ... }: {
                    services.clankers-daemon = {
                      enable = true;
                      package = fakeClankersPkg;
                      processManagement = {
                        enable = true;
                        defaultBackend = "systemd";
                        systemd.enable = true;
                        stateDir = "/srv/clankers/jobs";
                        logDir = "/var/log/clankers/jobs";
                        retention = {
                          maxAgeDays = 7;
                          maxRecords = 123;
                          maxLogBytes = 456789;
                        };
                      };
                    };
                  })
                ];
              };
              defaults = nixpkgs.lib.nixosSystem {
                inherit system;
                modules = [
                  self.nixosModules.clankers-daemon
                  ({ ... }: {
                    services.clankers-daemon = {
                      enable = true;
                      package = fakeClankersPkg;
                      stateDir = "/var/lib/clankers-defaults";
                      processManagement.enable = true;
                    };
                  })
                ];
              };
              customService = custom.config.systemd.services.clankers-daemon;
              defaultService = defaults.config.systemd.services.clankers-daemon;
              payload = builtins.toJSON {
                customEnvironment = customService.environment;
                customReadWritePaths = customService.serviceConfig.ReadWritePaths;
                customTmpfiles = custom.config.systemd.tmpfiles.rules;
                defaultEnvironment = defaultService.environment;
                defaultReadWritePaths = defaultService.serviceConfig.ReadWritePaths;
                defaultTmpfiles = defaults.config.systemd.tmpfiles.rules;
              };
            in pkgs.runCommand "nixos-module-process-persistence" {
              nativeBuildInputs = [ pkgs.jq ];
              passAsFile = [ "payload" ];
              inherit payload;
            } ''
              cp "$payloadPath" "$out"
              jq -e '
                .customEnvironment.CLANKERS_PROCESS_JOBS_ENABLED == "1" and
                .customEnvironment.CLANKERS_PROCESS_JOB_DEFAULT_BACKEND == "systemd" and
                .customEnvironment.CLANKERS_PROCESS_JOB_DB == "/srv/clankers/jobs/process-jobs.redb" and
                .customEnvironment.CLANKERS_PROCESS_JOB_REGISTRY_DIR == "/srv/clankers/jobs" and
                .customEnvironment.CLANKERS_PROCESS_JOB_LOG_DIR == "/var/log/clankers/jobs" and
                .customEnvironment.CLANKERS_PROCESS_JOB_RETENTION_MAX_AGE_DAYS == "7" and
                .customEnvironment.CLANKERS_PROCESS_JOB_RETENTION_MAX_RECORDS == "123" and
                .customEnvironment.CLANKERS_PROCESS_JOB_RETENTION_MAX_LOG_BYTES == "456789" and
                (.customReadWritePaths | index("/srv/clankers/jobs")) and
                (.customReadWritePaths | index("/var/log/clankers/jobs")) and
                (.customTmpfiles | index("d /srv/clankers/jobs 0750 clankers clankers -")) and
                (.customTmpfiles | index("d /var/log/clankers/jobs 0750 clankers clankers -")) and
                .defaultEnvironment.CLANKERS_PROCESS_JOB_DB == "/var/lib/clankers-defaults/process-jobs/process-jobs.redb" and
                .defaultEnvironment.CLANKERS_PROCESS_JOB_REGISTRY_DIR == "/var/lib/clankers-defaults/process-jobs" and
                .defaultEnvironment.CLANKERS_PROCESS_JOB_LOG_DIR == "/var/lib/clankers-defaults/process-jobs/logs" and
                .defaultEnvironment.CLANKERS_PROCESS_JOB_RETENTION_MAX_AGE_DAYS == "14" and
                .defaultEnvironment.CLANKERS_PROCESS_JOB_RETENTION_MAX_RECORDS == "1000" and
                .defaultEnvironment.CLANKERS_PROCESS_JOB_RETENTION_MAX_LOG_BYTES == "1073741824" and
                (.defaultReadWritePaths | index("/var/lib/clankers-defaults/process-jobs")) and
                (.defaultTmpfiles | index("d /var/lib/clankers-defaults/process-jobs 0750 clankers clankers -")) and
                (.defaultTmpfiles | index("d /var/lib/clankers-defaults/process-jobs/logs 0750 clankers clankers -"))
              ' "$out"
            '';
          nixos-module-process-pueue =
            let
              fakeClankersPkg = pkgs.writeShellScriptBin "clankers" "exit 0";
              enabled = nixpkgs.lib.nixosSystem {
                inherit system;
                modules = [
                  self.nixosModules.clankers-daemon
                  ({ ... }: {
                    services.clankers-daemon = {
                      enable = true;
                      package = fakeClankersPkg;
                      processManagement = {
                        enable = true;
                        defaultBackend = "pueue";
                        pueue = {
                          enable = true;
                          package = pkgs.pueue;
                          stateDir = "/var/lib/clankers/pueue-test";
                          groups = {
                            clankers = 3;
                            long = 1;
                          };
                        };
                      };
                    };
                  })
                ];
              };
              disabled = nixpkgs.lib.nixosSystem {
                inherit system;
                modules = [
                  self.nixosModules.clankers-daemon
                  ({ ... }: {
                    services.clankers-daemon = {
                      enable = true;
                      package = fakeClankersPkg;
                      processManagement = {
                        enable = true;
                        defaultBackend = "native";
                        pueue.enable = false;
                      };
                    };
                  })
                ];
              };
              enabledDaemon = enabled.config.systemd.services.clankers-daemon;
              disabledDaemon = disabled.config.systemd.services.clankers-daemon;
              payload = builtins.toJSON {
                enabledDaemon = {
                  inherit (enabledDaemon) after wants environment;
                };
                enabledPueued = {
                  inherit (enabled.config.systemd.services.clankers-pueued) environment;
                  serviceConfig = {
                    inherit (enabled.config.systemd.services.clankers-pueued.serviceConfig) ExecStart;
                  };
                };
                enabledSetup = {
                  inherit (enabled.config.systemd.services.clankers-pueue-setup) script;
                };
                enabledTmpfiles = enabled.config.systemd.tmpfiles.rules;
                disabledDaemon = {
                  inherit (disabledDaemon) environment;
                };
                disabledServices = builtins.attrNames disabled.config.systemd.services;
              };
            in pkgs.runCommand "nixos-module-process-pueue" {
              nativeBuildInputs = [ pkgs.jq ];
              passAsFile = [ "payload" ];
              inherit payload;
            } ''
              cp "$payloadPath" "$out"
              jq -e '
                .enabledDaemon.environment.CLANKERS_PROCESS_JOB_DEFAULT_BACKEND == "pueue" and
                .enabledDaemon.environment.CLANKERS_PROCESS_JOB_PUEUE_ENABLED == "1" and
                .enabledDaemon.environment.CLANKERS_PROCESS_JOB_PUEUE_GROUPS == "clankers,long" and
                .enabledDaemon.environment.PUEUE_CONFIG_PATH == "/var/lib/clankers/pueue-test/pueue.yml" and
                (.enabledDaemon.after | index("clankers-pueue-setup.service")) and
                (.enabledDaemon.wants | index("clankers-pueued.service")) and
                .enabledPueued.serviceConfig.ExecStart == "${pkgs.pueue}/bin/pueued" and
                .enabledPueued.environment.HOME == "/var/lib/clankers/pueue-test" and
                (.enabledSetup.script | contains("pueue group add clankers")) and
                (.enabledSetup.script | contains("pueue parallel --group clankers 3")) and
                (.enabledSetup.script | contains("pueue group add long")) and
                (.enabledSetup.script | contains("pueue parallel --group long 1")) and
                (.enabledTmpfiles | index("d /var/lib/clankers/pueue-test 0750 clankers clankers -")) and
                (.disabledDaemon.environment | has("CLANKERS_PROCESS_JOB_PUEUE_ENABLED") | not) and
                (.disabledServices | index("clankers-pueued") | not) and
                .disabledDaemon.environment.CLANKERS_PROCESS_JOB_DEFAULT_BACKEND == "native"
              ' "$out"
            '';
          nixos-module-process-systemd-limits =
            let
              fakeClankersPkg = pkgs.writeShellScriptBin "clankers" "exit 0";
              evaluated = nixpkgs.lib.nixosSystem {
                inherit system;
                modules = [
                  self.nixosModules.clankers-daemon
                  ({ ... }: {
                    services.clankers-daemon = {
                      enable = true;
                      package = fakeClankersPkg;
                      processManagement = {
                        enable = true;
                        defaultBackend = "systemd";
                        systemd = {
                          enable = true;
                          unitPrefix = "clankers-test";
                          memoryMax = "512M";
                          cpuQuota = "50%";
                          runtimeMaxSec = 600;
                          workingDirectory = "/srv/clankers/work";
                          writablePaths = [ "/srv/clankers/work" "/var/tmp/clankers-jobs" ];
                          killGraceSec = 9;
                        };
                      };
                    };
                  })
                ];
              };
              service = evaluated.config.systemd.services.clankers-daemon;
              payload = builtins.toJSON {
                environment = service.environment;
                readWritePaths = service.serviceConfig.ReadWritePaths;
              };
            in pkgs.runCommand "nixos-module-process-systemd-limits" {
              nativeBuildInputs = [ pkgs.jq ];
              passAsFile = [ "payload" ];
              inherit payload;
            } ''
              cp "$payloadPath" "$out"
              jq -e '
                .environment.CLANKERS_PROCESS_JOB_DEFAULT_BACKEND == "systemd" and
                .environment.CLANKERS_PROCESS_JOB_SYSTEMD_ENABLED == "1" and
                .environment.CLANKERS_PROCESS_JOB_SYSTEMD_UNIT_PREFIX == "clankers-test" and
                .environment.CLANKERS_PROCESS_JOB_SYSTEMD_MEMORY_MAX == "512M" and
                .environment.CLANKERS_PROCESS_JOB_SYSTEMD_CPU_QUOTA == "50%" and
                .environment.CLANKERS_PROCESS_JOB_SYSTEMD_RUNTIME_MAX_SEC == "600" and
                .environment.CLANKERS_PROCESS_JOB_SYSTEMD_WORKING_DIRECTORY == "/srv/clankers/work" and
                .environment.CLANKERS_PROCESS_JOB_SYSTEMD_WRITABLE_PATHS == "/srv/clankers/work:/var/tmp/clankers-jobs" and
                .environment.CLANKERS_PROCESS_JOB_SYSTEMD_KILL_GRACE_SEC == "9" and
                (.readWritePaths | index("/srv/clankers/work")) and
                (.readWritePaths | index("/var/tmp/clankers-jobs"))
              ' "$out"
            '';
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
