{ sources ? import ./nix/sources.nix
, nixpkgs ? import sources.nixpkgs {}
}:
{
  auto = with nixpkgs; rustPlatform.buildRustPackage rec {
    pname = "auto";
    version = "0.0.1";

    src =
      let
        whitelist = builtins.map builtins.toString [
          ./Cargo.toml
          ./Cargo.lock
          ./src
          ./src/autofile.rs
          ./src/main.rs
          ./src/queue.rs
        ];
        # Compute source based on whitelist
        whitelistedSrc = lib.cleanSourceWith {
          src = lib.cleanSource ./.;
          filter = path: _type: lib.elem path whitelist;
        };
      in
        whitelistedSrc;

    buildInputs = [sox];

    cargoSha256 = "0a42mkciysf3ywpnyf7dfli6zmr0kmljrxkc5i1hxmxkynvmjz77";
  };
}
