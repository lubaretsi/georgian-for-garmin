{
  description = "Georgian for Garmin: Georgian-extended Garmin watch fonts and their installer";

  # Pinned exactly: Georgian glyphs are rasterized with this Pillow/FreeType, and the 27 px
  # output must stay byte-identical to the build verified on hardware.
  inputs.nixpkgs.url = "tarball+https://codeload.github.com/NixOS/nixpkgs/tar.gz/f13ff45afd1bb73e640eaa08a7066dbed07e3238";

  outputs = { self, nixpkgs }:
    let
      inherit (nixpkgs) lib;
      # This nixpkgs no longer supports x86_64-darwin.
      systems = [ "aarch64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      # Every stock font pack that gets Georgian, and the font the glyphs come from.
      registry = builtins.fromJSON (builtins.readFile ./fonts/registry.json);
      stockParts = lib.concatMap (pack: pack.parts) registry.packs;

      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./builder
          ./fonts/registry.json
          ./fonts/registry.schema.json
        ];
      };

      fontInputs = pkgs: {
        GFG_STOCK_DIR = pkgs.linkFarm "garmin-stock-fonts" (map
          (part: {
            name = "${part.part}.fnt";
            path = pkgs.fetchurl { inherit (part) url sha256; };
          })
          stockParts);
        GFG_NOTO_SANS_GEORGIAN = pkgs.fetchurl { inherit (registry.georgian_font) url sha256; };
        GFG_NOTO_OFL = pkgs.fetchurl { inherit (registry.georgian_font.license) url sha256; };
      };

      python = pkgs: pkgs.python3.withPackages (ps: with ps; [ fonttools jsonschema pillow pytest ]);
    in
    {
      packages = forAllSystems (pkgs: {
        fonts = pkgs.runCommand "georgian-garmin-fonts"
          ((fontInputs pkgs) // {
            nativeBuildInputs = [ (python pkgs) ];
            PYTHONDONTWRITEBYTECODE = 1;
          })
          ''
            cd ${src}/builder
            python -m garmin_fonts --registry ../fonts/registry.json --stock-dir "$GFG_STOCK_DIR" \
              --noto "$GFG_NOTO_SANS_GEORGIAN" --license "$GFG_NOTO_OFL" --out "$out"
          '';
      });

      checks = forAllSystems (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) fonts;
        builder = pkgs.runCommand "georgian-garmin-fonts-tests"
          ((fontInputs pkgs) // {
            nativeBuildInputs = [ (python pkgs) ];
            PYTHONDONTWRITEBYTECODE = 1;
          })
          ''
            cd ${src}/builder
            pytest -q
            touch "$out"
          '';
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell ((fontInputs pkgs) // {
          GEORGIAN_FOR_GARMIN_FONTS = self.packages.${pkgs.stdenv.hostPlatform.system}.fonts;
          packages = with pkgs; [
            (python pkgs)
            cargo
            rustc
            rustfmt
            clippy
            cargo-tauri
            pkg-config
          ];
        });
      });
    };
}
