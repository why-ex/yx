{
  description = "yx - transparent Yocto/kas workflow frontend";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    pi-en = {
      url = "github:u2up/pi-en";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, pi-en }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          rustPackages = with pkgs; [
            cargo
            clippy
            rust-analyzer
            rustc
            rustfmt
          ];
        in
        {
          default = pkgs.mkShell {
            packages = rustPackages;

            RUST_BACKTRACE = "1";
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

            shellHook = ''
              echo "[yx] Rust development shell"
              echo "[yx] Try: cargo check --workspace"
            '';
          };

          agent = pi-en.lib.mkPiShell {
            inherit pkgs;
            includeCoordinationHelpers = true;

            extraPackages = rustPackages;

            shellHook = ''
              export RUST_BACKTRACE="1"
              export RUST_SRC_PATH="${pkgs.rustPlatform.rustLibSrc}"

              echo "[yx] Pi-en agent shell with Rust development tools"
              echo "[yx] Try: cargo check --workspace"
              echo "[yx] Pi tools: pien, pi-en, pi-en-shell"
            '';
          };
        });

      formatter = forAllSystems (system:
        nixpkgs.legacyPackages.${system}.nixpkgs-fmt);
    };
}
