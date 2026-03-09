{
  description = "clankers — Rust project built with unit2nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    unit2nix.url = "github:brittonr/unit2nix";
    crane.url = "github:ipetkov/crane";  # only used for WASM plugin vendoring
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, unit2nix, crane, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        # Nightly toolchain — needed for WASM plugin builds (-Zbuild-std)
        # and for the devShell.
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # ── Main workspace (unit2nix) ──────────────────────────────────────
        #
        # Regenerate build-plan.json whenever Cargo.lock changes:
        #   nix run github:brittonr/unit2nix -- --workspace
        ws = unit2nix.lib.${system}.buildFromUnitGraph {
          inherit pkgs;
          src = ./.;
          resolvedJson = ./build-plan.json;

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
        # workspace graph (the workspace uses `rpc` only). Separate plan:
        #   nix run github:brittonr/unit2nix -- -p clankers-router --features cli --include-dev -o build-plan-router.json
        wsRouter = unit2nix.lib.${system}.buildFromUnitGraph {
          inherit pkgs;
          src = ./.;
          resolvedJson = ./build-plan-router.json;

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

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        pluginVendorDir = craneLib.vendorMultipleCargoDeps {
            cargoConfigs = [];
            cargoLockParsedList =
              (map (p:
                builtins.fromTOML (builtins.readFile (./. + "/${p.dir}/Cargo.lock"))
              ) pluginSpecs)
              ++
              [ (builtins.fromTOML (builtins.readFile
                  "${rustToolchain}/lib/rustlib/src/rust/library/Cargo.lock")) ];
          };

        clankers-plugins = pkgs.stdenv.mkDerivation {
          pname = "clankers-plugins";
          version = "0.1.0";
          src = pluginSrc;
          nativeBuildInputs = [ rustToolchain pkgs.clang pkgs.mold ];

          configurePhase = ''
            cat ${pluginVendorDir}/config.toml >> .cargo/config.toml
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
            clankers-agent-defs
            clankers-auth
            clankers-db
            clankers-matrix
            clankers-merge
            clankers-model-selection
            clankers-procmon
            clankers-router
            clankers-specs
            clankers-tui
            clankers-tui-types
            clankers-zellij
            ;

          # Clippy — run via cargo since buildRustCrate's clippy wrapper
          # doesn't support custom rustc toolchains yet.  Deps are vendored
          # via crane so the check works inside the Nix sandbox.
          clippy =
            let
              vendorDir = craneLib.vendorCargoDeps { src = ./.; };
            in
            pkgs.runCommand "cargo-clippy-check" {
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
              src = ./.;
            } ''
              export HOME=$(mktemp -d)
              cp -r $src source && chmod -R u+w source && cd source
              mkdir -p .cargo
              cp ${vendorDir}/config.toml .cargo/config.toml
              cargo clippy --all-targets -- -D warnings
              touch $out
            '';

          # Format check
          fmt = pkgs.runCommand "cargo-fmt-check" {
            nativeBuildInputs = [ rustToolchain ];
            src = ./.;
          } ''
            cd $src
            cargo fmt --check
            touch $out
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
