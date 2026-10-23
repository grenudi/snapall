{
  description = "snapall: snapshot every snapper config in one command";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
    in
    {
      packages = forAll (pkgs: rec {
        snapall = pkgs.rustPlatform.buildRustPackage {
          pname = "snapall";
          inherit version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "snapall" ];
          # The tests need `cargo metadata` and, for the real-snapper ones, root and Btrfs.
          # CI runs them; the package build does not.
          doCheck = false;
          nativeBuildInputs = [ pkgs.makeWrapper ];
          # snapall drives snapper: make sure it is found whatever the user's PATH.
          postInstall = ''
            wrapProgram $out/bin/snapall --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.snapper ]}
          '';
          meta = {
            description = "Snapshot every snapper config in one command, from the CLI or a terminal UI";
            homepage = "https://github.com/grenudi/snapall";
            license = pkgs.lib.licenses.gpl3Plus;
            mainProgram = "snapall";
            platforms = pkgs.lib.platforms.linux;
          };
        };
        default = snapall;
      });

      apps = forAll (pkgs: rec {
        snapall = { type = "app"; program = "${self.packages.${pkgs.system}.snapall}/bin/snapall"; };
        default = snapall;
      });
    };
}
