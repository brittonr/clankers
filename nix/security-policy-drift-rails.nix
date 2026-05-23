{ pkgs, rustToolchain, src }:

let
  scriptBins = [
    "check-steel-self-mutation-policy"
    "check-steel-default-orchestration"
    "check-steel-turn-planning-ucan-authority"
    "check-embedded-lego-contracts"
  ];

  scriptPackageSrc = pkgs.runCommand "security-policy-drift-rails-src" { } ''
    mkdir -p "$out/src/bin"

    cp "${./security-policy-drift-rails.Cargo.lock}" "$out/Cargo.lock"

    cat > "$out/Cargo.toml" <<'EOF'
    [package]
    name = "security-policy-drift-rails"
    version = "0.1.0"
    edition = "2024"

    [dependencies]
    blake3 = "1"
    serde = { version = "1", features = ["derive"] }
    serde_json = "1"
    EOF

    strip_cargo_script() {
      in_script="$1"
      out_rs="$2"
      awk '
        NR == 1 && /^#!/ { next }
        /^---cargo$/ { in_frontmatter = 1; next }
        in_frontmatter && /^---$/ { in_frontmatter = 0; next }
        !in_frontmatter { print }
      ' "$in_script" > "$out_rs"
    }

    for name in ${pkgs.lib.escapeShellArgs scriptBins}; do
      strip_cargo_script "${src}/scripts/$name.rs" "$out/src/bin/$name.rs"
    done
  '';
in
pkgs.rustPlatform.buildRustPackage {
  pname = "security-policy-drift-rails";
  version = "0.1.0";
  src = scriptPackageSrc;

  cargoHash = "sha256-wxfDypBGXWCbG15Qs71V44tS+Ktigitvm+gDmr0i08A=";

  nativeBuildInputs = [ rustToolchain ];
  cargo = rustToolchain;
  rustc = rustToolchain;

  doCheck = false;
  doInstallCheck = true;

  installCheckPhase = ''
    runHook preInstallCheck

    cp -R ${src} "$TMPDIR/source"
    chmod -R u+w "$TMPDIR/source"
    cd "$TMPDIR/source"

    for name in ${pkgs.lib.escapeShellArgs scriptBins}; do
      "$out/bin/$name"
    done

    runHook postInstallCheck
  '';
}
