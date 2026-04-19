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
      url = "github:brittonr/subwayrat";
      flake = false;
    };

    # subwayrat itself depends on ratcore via ../ratcore.
    ratcore-src = {
      url = "github:brittonr/ratcore";
      flake = false;
    };

    # Plugin SDK used by all WASM plugins.
    clanker-plugin-sdk-src = {
      url = "github:brittonr/clanker-plugin-sdk";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, unit2nix, rust-overlay, flake-utils, subwayrat-src, ratcore-src, clanker-plugin-sdk-src, ... }:
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

        # Strip openspec path dep — not published yet, and cargo resolves
        # all path deps at manifest load time even when optional/disabled.
        workspaceSrc = pkgs.runCommand "clankers-workspace-src" {} ''
          cp -r ${./.} $out
          chmod -R u+w $out
          sed -i '/^openspec = { path = /d' $out/Cargo.toml
          sed -i 's/openspec = \["dep:openspec"\]//' $out/Cargo.toml
          sed -i 's/"openspec", //' $out/Cargo.toml
          sed -i '/^openspec = { path = /d' $out/crates/clankers-agent/Cargo.toml
          sed -i 's/openspec = \["dep:openspec"\]//' $out/crates/clankers-agent/Cargo.toml
          sed -i 's/"openspec"//' $out/crates/clankers-agent/Cargo.toml
        '';

        # ── Main workspace (unit2nix auto mode) ─────────────────────────
        ws = unit2nix.lib.${system}.buildFromUnitGraphAuto {
          inherit pkgs rustToolchain;
          src = workspaceSrc;
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
          src = ./vendor/clanker-router;
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

        # ── Extracted derivations ───────────────────────────────────────
        clankersPkg = ws.workspaceMembers."clankers".build;
        verus = import ./nix/verus.nix { inherit pkgs; };
        clankers-docs = import ./nix/docs.nix { inherit pkgs; src = ./.; };
        clankers-plugins = import ./nix/plugins.nix {
          inherit pkgs rustToolchain unit2nix system clanker-plugin-sdk-src;
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
