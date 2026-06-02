{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs {
          inherit system overlays;
          config = {
            allowUnfree = true;
          };
        };
        rustPlatform = pkgs.rust.packages.stable.rustPlatform;
      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            nativeBuildInputs = [
              # nix develop shells will by default include a bash in the $PATH,
              # however this bash will be a non-interactive bash. The deviates from
              # how nix-shell works. This fix was taken from:
              #    https://discourse.nixos.org/t/interactive-bash-with-nix-develop-flake/15486
              bashInteractive

              # Rust
              # (rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
              #   extensions = [ "rust-src" "miri" ];
              #   targets = [ "x86_64-unknown-linux-gnu" ];
              # }))

              (rust-bin.nightly."2026-02-27".default.override {
                extensions = [ "rust-src" ];
                targets = [
                  "x86_64-unknown-linux-gnu"
                  "x86_64-unknown-linux-musl"
                ];
              })

              cargo-deny
              cargo-tarpaulin

            ];

            RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";

            shellHook = ''
              # nix develop shells will by default overwrite the $SHELL variable with a
              # non-interactive version of bash. The deviates from how nix-shell works.
              # This fix was taken from:
              #    https://discourse.nixos.org/t/interactive-bash-with-nix-develop-flake/15486
              #
              # See also: nixpkgs#5131 nixpkgs#6091
              export SHELL=${pkgs.bashInteractive}/bin/bash
              alias jq=xq
            '';

            packages = with pkgs; [
              claude-code
              (rustPlatform.buildRustPackage (finalAttrs: {
                pname = "xq";
                version = "0.5.0";
                cargoHash = "sha256-yK8yCYiFM14aBem65/3eWPa+Ym18/gxU5dw4mbLtLnc=";
                src = fetchFromGitHub {
                  owner = "MiSawa";
                  repo = "xq";
                  hash = "sha256-zoGMkYLeE+kNEzFd1hICAJ157wwH33G6pd3Ht90kI9I=";
                  rev = "86c2e322fe3094be1ea336abd8841bb64777a6cd";
                };
              }))
            ];
          };
      }
    );
}
