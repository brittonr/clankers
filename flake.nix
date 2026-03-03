{
  description = "clankers — Rust project built with Crane";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Common source filtering
        src = craneLib.cleanCargoSource ./.;

        # Plugin source includes plugin.json manifests
        pluginSrc = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (builtins.match ".*plugin\\.json$" path != null)
            || (craneLib.filterCargoSources path type);
        };

        # Common build inputs
        nativeBuildInputs = with pkgs; [
          pkg-config
          clang
          mold
        ];

        buildInputs = with pkgs; [
          openssl
          sqlite
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
        ];

        # Build just the cargo dependencies for caching
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src nativeBuildInputs buildInputs;
        };

        # Build the actual package
        clankers = craneLib.buildPackage {
          inherit src cargoArtifacts nativeBuildInputs buildInputs;
        };

        # Build just the clankers-router binary (with CLI/TUI feature)
        clankers-router = craneLib.buildPackage {
          inherit src cargoArtifacts nativeBuildInputs buildInputs;
          pname = "clankers-router";
          cargoExtraArgs = "-p clankers-router --features cli";
        };

        # WASM plugin builds
        pluginSpecs = [
          { dir = "plugins/clankers-hash"; name = "clankers_hash"; }
          { dir = "plugins/clankers-self-validate"; name = "clankers_self_validate"; }
          { dir = "plugins/clankers-test-plugin"; name = "clankers_test_plugin"; }
          { dir = "plugins/clankers-text-stats"; name = "clankers_text_stats"; }
          { dir = "examples/plugins/clankers-wordcount"; name = "clankers_wordcount"; }
        ];

        pluginVendorDir = craneLib.vendorMultipleCargoDeps {
          cargoConfigs = [];
          cargoLockParsedList =
            # Plugin lockfiles
            (map (p:
              builtins.fromTOML (builtins.readFile (./. + "/${p.dir}/Cargo.lock"))
            ) pluginSpecs)
            ++
            # Std library deps (needed for -Zbuild-std)
            [ (builtins.fromTOML (builtins.readFile
                "${rustToolchain}/lib/rustlib/src/rust/library/Cargo.lock")) ];
        };

        clankers-plugins = pkgs.stdenv.mkDerivation {
          pname = "clankers-plugins";
          version = "0.1.0";
          src = pluginSrc;
          nativeBuildInputs = [ rustToolchain pkgs.clang pkgs.mold ];

          configurePhase = ''
            # Append vendored dependency config to existing .cargo/config.toml
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
          default = clankers;
          inherit clankers clankers-router clankers-plugins;
        };

        checks = {
          inherit clankers;

          # Run tests with nextest
          nextest = craneLib.cargoNextest {
            inherit src cargoArtifacts nativeBuildInputs buildInputs;
            partitions = 1;
            partitionType = "count";
          };

          # Clippy lints
          clippy = craneLib.cargoClippy {
            inherit src cargoArtifacts nativeBuildInputs buildInputs;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          };

          # Format check
          fmt = craneLib.cargoFmt {
            inherit src;
          };
        };

        devShells.default = craneLib.devShell {
          inherit buildInputs;

          packages = with pkgs; [
            cargo-nextest
            cargo-watch
            rust-analyzer

            # Allwinner / SDWire tooling
            sunxi-tools
            sd-mux-ctrl
            usbutils
          ];

          # Ensure the nightly toolchain is available
          inputsFrom = [ clankers ];

          # Put cargo build output on PATH so clankers can auto-start clankers-router
          shellHook = ''
            export PATH="$PWD/target/debug:$PATH"
            export LIBRARY_PATH="${pkgs.sqlite.out}/lib''${LIBRARY_PATH:+:$LIBRARY_PATH}"
          '';
        };
      }
    );
}
