{ lib, rustPlatform, cmake, pkg-config, git }:
rustPlatform.buildRustPackage {
  pname = "pinst";
  version = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).package.version;
  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      let name = baseNameOf path; in
      !(builtins.elem name [ "target" ".git" "result" "result-toolchain" ])
      && lib.cleanSourceFilter path type;
  };
  cargoLock.lockFile = ../Cargo.lock;
  nativeBuildInputs = [ cmake pkg-config ];
  nativeCheckInputs = [ git ];
  meta = {
    description = "Toolchain and dotfiles manager";
    mainProgram = "pinst";
    platforms = lib.platforms.unix;
  };
}
