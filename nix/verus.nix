# Verus verifier — prebuilt binary (x86_64-linux only).
#
# Verus requires a specific Rust toolchain + Z3. The prebuilt release bundles
# everything: verus binary, rust_verify, z3, proc-macro libs, and vstd source.
#
# rust_verify links against librustc_driver from Rust 1.93.1, so we provide
# that toolchain's libraries for autoPatchelf.
{ pkgs }:
let
  rustToolchain = pkgs.rust-bin.stable."1.93.1".default.override {
    extensions = [ "rustc-dev" "llvm-tools" ];
  };
in
pkgs.stdenv.mkDerivation {
  pname = "verus";
  version = "0.2026.03.10.13c14a1";

  src = pkgs.fetchzip {
    url = "https://github.com/verus-lang/verus/releases/download/release/0.2026.03.10.13c14a1/verus-0.2026.03.10.13c14a1-x86-linux.zip";
    hash = "sha256-tmlV/ozVX1GRuiEKh6qeFh61TGZSULVRwEvPNoiPgMM=";
  };

  nativeBuildInputs = [ pkgs.autoPatchelfHook pkgs.makeWrapper ];
  buildInputs = [ pkgs.stdenv.cc.cc.lib rustToolchain ];

  installPhase = ''
    runHook preInstall
    mkdir -p $out/bin $out/lib/verus
    cp -r . $out/lib/verus/

    # Upstream `verus` binary checks for rustup. On NixOS we bypass it:
    # call rust_verify directly with the right library paths and Z3.
    makeWrapper $out/lib/verus/rust_verify $out/bin/verus \
      --set VERUS_Z3_PATH "$out/lib/verus/z3" \
      --prefix LD_LIBRARY_PATH : "${rustToolchain}/lib" \
      --prefix LD_LIBRARY_PATH : "${rustToolchain}/lib/rustlib/x86_64-unknown-linux-gnu/lib" \
      --add-flags "-L dependency=$out/lib/verus" \
      --add-flags "--extern builtin=$out/lib/verus/libverus_builtin.rlib" \
      --add-flags "--extern vstd=$out/lib/verus/libvstd.rlib" \
      --add-flags "--extern builtin_macros=$out/lib/verus/libverus_builtin_macros.so" \
      --add-flags "--extern state_machines_macros=$out/lib/verus/libverus_state_machines_macros.so" \
      --add-flags "--edition 2021"

    ln -s $out/lib/verus/cargo-verus $out/bin/cargo-verus
    runHook postInstall
  '';
}
