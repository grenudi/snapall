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
          nativeBuildInputs = [ pkgs.makeWrapper pkgs.installShellFiles ];
          # snapall drives snapper: make sure it is found whatever the user's PATH.
          postInstall = ''
            # Completions and the man page are generated from the contract the binary was built with.
            installShellCompletion --cmd snapall \
              --bash <($out/bin/snapall completions bash) \
              --zsh <($out/bin/snapall completions zsh) \
              --fish <($out/bin/snapall completions fish) \
              --nushell <($out/bin/snapall completions nushell)
            $out/bin/snapall man > snapall.1
            installManPage snapall.1
            install -Dm644 ${./packaging/desktop/snapall.desktop} $out/share/applications/snapall.desktop
            install -Dm644 ${./packaging/desktop/snapall.svg} $out/share/icons/hicolor/scalable/apps/snapall.svg
            install -Dm755 ${./packaging/filemanagers/snapall-file-menu} $out/bin/snapall-file-menu
            install -Dm644 ${./packaging/filemanagers/snapall.kio-servicemenu.desktop} $out/share/kio/servicemenus/snapall.desktop
            install -Dm644 ${./packaging/filemanagers/snapall.nemo_action} $out/share/nemo/actions/snapall.nemo_action
            install -Dm644 ${./packaging/filemanagers/snapall.libfm-action.desktop} $out/share/file-manager/actions/snapall.desktop
            install -Dm644 ${./packaging/filemanagers/snapall-nautilus.py} $out/share/nautilus-python/extensions/snapall.py
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
