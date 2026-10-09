{
  description = "pinst: locked CLI packages and Home Manager environments";

  # 26.05 retains Intel macOS support; 26.11/unstable does not.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  inputs.home-manager = {
    url = "github:nix-community/home-manager/release-26.05";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, home-manager }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      pkgsFor = system: import nixpkgs { inherit system; };
    in {
      homeConfigurations = nixpkgs.lib.mapAttrs (_: host:
        home-manager.lib.homeManagerConfiguration {
          pkgs = pkgsFor host.system;
          modules = [ ./nix/home.nix {
            home.username = host.username;
            home.homeDirectory = host.homeDirectory;
            home.sessionVariables = host.sessionVariables or { };
          } ];
        }) (import ./nix/hosts.nix);
      packages = forAllSystems (system:
        let
          pkgs = pkgsFor system;
          pinst = pkgs.callPackage ./nix/pinst.nix { };
          tools = import ./nix/packages.nix { inherit pkgs; };
        in {
          inherit pinst;
          default = pinst;
          toolchain = pkgs.buildEnv {
            name = "pinst-toolchain";
            paths = tools;
            pathsToLink = [ "/bin" "/share/man" "/share/zsh" ];
          };
        });
      devShells = forAllSystems (system:
        let pkgs = pkgsFor system; in {
          default = pkgs.mkShell {
            packages = with pkgs; [ cargo rustc rustfmt clippy cmake pkg-config just python3 ];
          };
        });
    };
}
