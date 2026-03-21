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

    # Standalone source for the clanker-router CLI binary.
    # The workspace uses it as a library dep; this builds the CLI.
    clanker-router-src = {
      url = "github:brittonr/clanker-router";
      flake = false;
    };

    # rat-* TUI crates used by clankers-tui. The workspace Cargo.toml
    # references these as path deps (../subwayrat/...); we pin them here
    # and patch the source so they resolve inside the Nix sandbox.
    subwayrat-src = {
      url = "github:brittonr/subwayrat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, unit2nix, rust-overlay, flake-utils, clanker-router-src, subwayrat-src, ... }:
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
          inherit system;
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
          # rat-* TUI crates live in a sibling repo
          externalSources = { "../subwayrat" = subwayrat-src; };
          buildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
            rustc = rustToolchain;
          };
          extraCrateOverrides = {
            aws-lc-rs = attrs: {
              nativeBuildInputs = [ pkgs.cmake pkgs.go ];
            };
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
          src = clanker-router-src;
          features = "cli";
          buildRustCrateForPkgs = pkgs: pkgs.buildRustCrate.override {
            rustc = rustToolchain;
          };
          extraCrateOverrides = {
            aws-lc-rs = attrs: {
              nativeBuildInputs = [ pkgs.cmake pkgs.go ];
            };
          };
        };

        # ── Extracted derivations ───────────────────────────────────────
        clankersPkg = ws.workspaceMembers."clankers".build;
        verus = import ./nix/verus.nix { inherit pkgs; };
        clankers-docs = import ./nix/docs.nix { inherit pkgs; src = ./.; };
        clankers-plugins = import ./nix/plugins.nix {
          inherit pkgs rustToolchain unit2nix system;
          src = ./.;
        };
        routerPkg = routerBuild.rootCrate.build;
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
