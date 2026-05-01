{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        ciToolchain = toolchain.override {
          targets = [
            "x86_64-unknown-linux-gnu"
            "aarch64-apple-darwin"
            "x86_64-apple-darwin"
          ];
        };
        commonBuildInputs = [
          pkgs.cargo-edit
          pkgs.cargo-watch
          pkgs.nodejs_22
          pkgs.openssh
          pkgs.pnpm
          pkgs.biome
          pkgs.tea
          pkgs.just
          pkgs.rsync
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "vimdoc-language-server";
          version = "0.0.1";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          postInstall = ''
            install -Dm644 man/*.1 -t $out/share/man/man1
          '';
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [ toolchain ] ++ commonBuildInputs;
        };

        devShells.ci = pkgs.mkShell {
          buildInputs = [ ciToolchain ] ++ commonBuildInputs;
        };
      }
    );
}
