{
  description = "A devShell example";

  inputs = {
    nixpkgs.url      = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustToolchain = pkgs.rust-bin.beta.latest.default.override {
          extensions = [ "rust-analyzer" ];
        };
      in
      with pkgs;
      {
        # rust-bin.stable.latest.default.override {
        #   extensions = [ "rust-analyzer" ];
        # };
        devShells.default = mkShell {
          nativeBuildInputs = [
            rustToolchain
            pkg-config
            SDL2
            jujutsu
          ];
        };
      }
    );
}
