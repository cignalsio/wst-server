{
  pkgs,
  ...
}:
let
  src = ../.;
  pname = "wst";
  version = "0.1.0";
in
pkgs.rustPlatform.buildRustPackage {
  inherit
    src
    pname
    version
    ;

  cargoLock.lockFile = ../Cargo.lock;
}
