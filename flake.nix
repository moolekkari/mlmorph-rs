{
  description = "mlmorph-rs clean-room spike and grammar build tools";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          sfst = pkgs.stdenv.mkDerivation {
            pname = "sfst-tools";
            version = "1.4.7g";
            src = pkgs.fetchurl {
              url = "https://www.cis.lmu.de/~schmid/tools/SFST/data/SFST-1.4.7g.zip";
              hash = "sha256-Xzqy2BkNyTGBO1ujz5THIBPM578D4W1/su2oa9mc6UQ=";
            };
            nativeBuildInputs = [ pkgs.unzip pkgs.flex pkgs.bison ];
            postUnpack = "sourceRoot=$sourceRoot/src";
            makeFlags = [ "READLINE=" "LREADLINE=" ];
            buildPhase = ''
              runHook preBuild
              make -j$NIX_BUILD_CORES READLINE= LREADLINE= fst-compiler-utf8 fst-print fst-infl fst-infl2 fst-compact
              runHook postBuild
            '';
            installPhase = ''
              mkdir -p $out/bin
              cp fst-compiler-utf8 fst-print fst-infl fst-infl2 fst-compact $out/bin/
            '';
            meta.license = pkgs.lib.licenses.gpl2Plus;
          };
          shell = pkgs.mkShell {
            packages = with pkgs; [ rustc cargo clippy rustfmt sfst gnumake git python3 gzip ];
          };
        in { default = shell; ci = shell; });
    };
}
