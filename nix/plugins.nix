# WASM plugin builds.
#
# Plugins are standalone crates with their own Cargo.lock, built to
# wasm32-unknown-unknown with -Zbuild-std. unit2nix doesn't handle WASM
# targets, so these use a plain stdenv derivation.
#
# Git dependencies (clanker-plugin-sdk) are vendored from a flake input
# alongside crates.io deps so the build runs fully offline in the sandbox.
{ pkgs, rustToolchain, unit2nix, system, src, clanker-plugin-sdk-src }:
let
  specs = [
    { dir = "plugins/clankers-calendar";              name = "clankers_calendar"; }
    { dir = "plugins/clankers-email";                 name = "clankers_email"; }
    { dir = "plugins/clankers-github";                name = "clankers_github"; }
    { dir = "plugins/clankers-hash";                  name = "clankers_hash"; }
    { dir = "plugins/clankers-self-validate";         name = "clankers_self_validate"; }
    { dir = "plugins/clankers-test-plugin";           name = "clankers_test_plugin"; }
    { dir = "plugins/clankers-text-stats";            name = "clankers_text_stats"; }
    { dir = "examples/plugins/clankers-wordcount";    name = "clankers_wordcount"; }
  ];

  pluginSrc = pkgs.lib.cleanSourceWith {
    inherit src;
    filter = path: type:
      (builtins.match ".*plugin\\.json$" path != null)
      || (builtins.match ".*\\.(rs|toml|lock)$" path != null)
      || type == "directory";
  };

  vendor = unit2nix.lib.${system}.vendorMultipleCargoDeps {
    inherit pkgs;
    cargoLocks =
      (map (p: src + "/${p.dir}/Cargo.lock") specs)
      ++ [ "${rustToolchain}/lib/rustlib/src/rust/library/Cargo.lock" ];
  };

  # Augmented vendor directory: crates.io deps + git-sourced plugin SDK.
  augmentedVendor = pkgs.runCommand "augmented-vendor" {} ''
    sdk_vendor_dir=$(grep 'directory' ${vendor.cargoConfig} | cut -d'"' -f2)
    cp -r $sdk_vendor_dir $out
    chmod -R u+w $out
    cp -r ${clanker-plugin-sdk-src} $out/clanker-plugin-sdk-0.1.0
    chmod -R u+w $out/clanker-plugin-sdk-0.1.0
    echo '{"files":{}}' > $out/clanker-plugin-sdk-0.1.0/.cargo-checksum.json
  '';

  # Cargo config that redirects both crates-io and the git SDK to our vendor dir.
  cargoConfig = pkgs.writeText "cargo-config.toml" ''
    [source.crates-io]
    replace-with = "vendored-sources"

    [source."git+https://github.com/brittonr/clanker-plugin-sdk"]
    git = "https://github.com/brittonr/clanker-plugin-sdk"
    replace-with = "vendored-sources"

    [source.vendored-sources]
    directory = "${augmentedVendor}"
  '';

  # Writable copy of the source with cargo configs injected.
  preparedSrc = pkgs.runCommand "clankers-plugin-src" {} ''
    cp -r ${pluginSrc} $out
    chmod -R u+w $out
    # Cargo searches ancestors of CWD for .cargo/config.toml, not the
    # manifest directory. Put the config at the source root.
    mkdir -p $out/.cargo
    cp ${cargoConfig} $out/.cargo/config.toml
  '';
in
pkgs.stdenv.mkDerivation {
  pname = "clankers-plugins";
  version = "0.1.0";
  src = preparedSrc;
  nativeBuildInputs = [ rustToolchain pkgs.clang pkgs.mold ];

  # No configure needed — source arrives with cargo configs already injected.
  configurePhase = "true";

  buildPhase = ''
    runHook preBuild
    ${pkgs.lib.concatMapStringsSep "\n" (p: ''
      echo "Building ${p.name}…"
      cargo build \
        --manifest-path ${p.dir}/Cargo.toml \
        --target wasm32-unknown-unknown \
        --release \
        -Zbuild-std=std,panic_abort
    '') specs}
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
    '') specs}
    runHook postInstall
  '';
}
