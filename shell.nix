let
  sources = import ./nix/sources.nix;
  nixpkgs = import sources.nixpkgs {};
in
  nixpkgs.mkShell {
    name = "auto-dev";
    nativeBuildInputs = with nixpkgs; [
      rustc
      cargo
      rustfmt
      cargo-audit
      niv
    ];
    # Always enable rust backtraces in development shell
    RUST_BACKTRACE = "1";

    # Provide sources for rust-analyzer, because nixpkgs rustc doesn't include them in the sysroot
    RUST_SRC_PATH = "${nixpkgs.rustPlatform.rustcSrc}";
  }
