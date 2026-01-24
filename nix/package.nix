{
  pkgs,
  ...
}:
let
  pname = "wst";
  version = "0.1.0";
  src = pkgs.lib.cleanSource ../.;
in
pkgs.rustPlatform.buildRustPackage {
  inherit
    pname
    version
    src
    ;

  cargoLock.lockFile = ../Cargo.lock;
}
