# Documentation site — runs docs/generate.sh then mdbook.
{ pkgs, src }:
pkgs.stdenv.mkDerivation {
  pname = "clankers-docs";
  version = "0.1.0";

  src = pkgs.lib.cleanSourceWith {
    inherit src;
    filter = path: type:
      (builtins.match ".*\\.(rs|toml|lock|md|css|json|sh)$" path != null)
      || type == "directory";
  };

  nativeBuildInputs = [ pkgs.mdbook ];

  buildPhase = ''
    runHook preBuild
    bash docs/generate.sh "$PWD"
    mdbook build docs
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    cp -r docs/book $out
    runHook postInstall
  '';
}
