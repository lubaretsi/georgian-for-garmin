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

      # Garmin's Armenian Noto Sans Regular 2.10 ExtData fonts (21, 24, 27 and 31 px).
      stockFonts = [
        { part = "006-D9035-01"; release = "67325442-1a85-4c5d-84b9-96a01cb825b7"; sha256 = "575b132a8f241c42604cfa9e7809ec8f3a51b0b6e32800c37ac24d404c707ddf"; }
        { part = "006-D9035-02"; release = "3c0cffa0-0a57-44b1-809d-fb28b09fd0cd"; sha256 = "c991505c121c7bf7fc803790c1c838b1288a33a9d2a3147f0c59168a938015eb"; }
        { part = "006-D9035-03"; release = "04d51b36-9d5f-4273-8195-b263a069931f"; sha256 = "bb61a1a45c90f7526444ce58356691c43314899e290b33232acbf5b0ed79ea1d"; }
        { part = "006-D9035-04"; release = "3ab8292f-8c9c-435e-9406-980de0320426"; sha256 = "5cc7fa6d5b7a538eed25581e65abc6e3fad09ccdeab026097b8aed643b336837"; }
      ];

      builderSrc = lib.fileset.toSource {
        root = ./builder;
        fileset = ./builder;
      };

      fontInputs = pkgs: {
        GFG_STOCK_DIR = pkgs.linkFarm "garmin-armenian-stock-fonts" (map
          (font: {
            name = "${font.part}.fnt";
            path = pkgs.fetchurl {
              url = "https://download.garmin.com/gsup/${font.part}/2.10/full/rel/${font.release}/${font.part}.fnt";
              inherit (font) sha256;
            };
          })
          stockFonts);
        # Noto Sans Georgian 2.005 variable font as served by Google Fonts (v44).
        GFG_NOTO_SANS_GEORGIAN = pkgs.fetchurl {
          name = "NotoSansGeorgian-VariableFont_wdth_wght.ttf";
          url = "https://fonts.gstatic.com/s/notosansgeorgian/v44/PlIgFke5O6RzLfvNNVSitxkr76PRHBCiaf5GGPq86g.ttf";
          sha256 = "fbc282fa1995b77383a66ea3131e2dfee5fc03233a279ce36edc84b2e801c80e";
        };
        # The OFL from google/fonts at the commit that added Noto Sans Georgian 2.005.
        GFG_NOTO_OFL = pkgs.fetchurl {
          name = "NotoSansGeorgian-OFL.txt";
          url = "https://raw.githubusercontent.com/google/fonts/07eb586093156484de83c97a3a565f8b1e757af2/ofl/notosansgeorgian/OFL.txt";
          sha256 = "8c02263c5d73d40544f9ed91e30c4e947407057a3cc430d7b786189aeceff6df";
        };
      };

      python = pkgs: pkgs.python3.withPackages (ps: with ps; [ fonttools pillow pytest ]);
    in
    {
      packages = forAllSystems (pkgs: {
        fonts = pkgs.runCommand "georgian-garmin-fonts"
          ((fontInputs pkgs) // {
            nativeBuildInputs = [ (python pkgs) ];
            PYTHONDONTWRITEBYTECODE = 1;
          })
          ''
            cd ${builderSrc}
            python -m garmin_fonts --stock-dir "$GFG_STOCK_DIR" --noto "$GFG_NOTO_SANS_GEORGIAN" \
              --license "$GFG_NOTO_OFL" --out "$out"
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
            cd ${builderSrc}
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
