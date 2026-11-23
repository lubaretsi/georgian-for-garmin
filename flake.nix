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

      # Every stock font pack that gets Georgian, and the font the glyphs come from. Bitmap
      # parts are `<part>.fnt`, TrueType parts `<part>.bin`, as Garmin names the downloads.
      registry = builtins.fromJSON (builtins.readFile ./fonts/registry.json);
      suffix = { bitmap = "fnt"; truetype = "bin"; };
      stockParts = lib.concatMap
        (pack: map (part: part // { file = "${part.part}.${suffix.${pack.kind}}"; }) pack.parts)
        registry.packs;
      # Other pinned stock fonts (the registry's `stock`): the codec's round-trip tests and the
      # toolkit's experiments use them; the app's fonts don't.
      catalogParts = map (part: part // { file = "${part.part}.fnt"; }) registry.stock;

      builderFiles = lib.fileset.unions [
        ./builder
        ./fonts/registry.json
        ./fonts/registry.schema.json
      ];
      src = lib.fileset.toSource { root = ./.; fileset = builderFiles; };
      # The builder tests also check the per-part size table in fonts/README.md and the watch
      # table in README.md, and render the test text over every watch report fixture with the
      # fallback models.
      testSrc = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          builderFiles
          ./fonts/README.md
          ./fonts/fallback-models.json
          ./fonts/fallback-models.schema.json
          ./README.md
          ./crates/core/tests/reports
        ];
      };

      farm = pkgs: name: parts: pkgs.linkFarm name (map
        (part: {
          name = part.file;
          path = pkgs.fetchurl { inherit (part) url sha256; };
        })
        parts);

      fontInputs = pkgs: {
        GFG_STOCK_DIR = farm pkgs "garmin-stock-fonts" stockParts;
        GFG_NOTO_SANS_GEORGIAN = pkgs.fetchurl { inherit (registry.georgian_font) url sha256; };
        GFG_NOTO_OFL = pkgs.fetchurl { inherit (registry.georgian_font.license) url sha256; };
      };
      # For the builder tests and the dev shell (python -m garmin_fonts fetch finds them here).
      catalogInputs = pkgs: (fontInputs pkgs) // {
        GFG_CATALOG_DIR = farm pkgs "garmin-catalog-fonts" catalogParts;
      };

      python = pkgs: pkgs.python3.withPackages (ps: with ps; [ fonttools jsonschema pillow pytest ]);

      # Pre-release versions (with a `-` identifier, e.g. 0.3.0-beta.1) build fonts with beta
      # test marks (fonts/README.md, Test marks); releases build the fonts tested on watches.
      version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
      preRelease = lib.hasInfix "-" version;

      buildFonts = pkgs: testMarks: pkgs.runCommand
        "georgian-garmin-fonts${lib.optionalString testMarks "-test-marks"}"
        ((fontInputs pkgs) // {
          nativeBuildInputs = [ (python pkgs) ];
          PYTHONDONTWRITEBYTECODE = 1;
        })
        ''
          cd ${src}/builder
          python -m garmin_fonts build --registry ../fonts/registry.json --stock-dir "$GFG_STOCK_DIR" \
            --noto "$GFG_NOTO_SANS_GEORGIAN" --license "$GFG_NOTO_OFL" --out "$out" \
            ${lib.optionalString testMarks "--test-marks"}
        '';
    in
    {
      packages = forAllSystems (pkgs: {
        # The fonts the app embeds: test marks exactly when the workspace version is a pre-release.
        fonts = buildFonts pkgs preRelease;
        fonts-stable = buildFonts pkgs false;
        fonts-marked = buildFonts pkgs true;
      });

      checks = forAllSystems (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) fonts;
        # Its output holds the render report's PNGs (render/), which CI uploads.
        builder = pkgs.runCommand "georgian-garmin-fonts-tests"
          ((catalogInputs pkgs) // {
            nativeBuildInputs = [ (python pkgs) ];
            PYTHONDONTWRITEBYTECODE = 1;
            GFG_OFFLINE = 1;
          })
          ''
            mkdir -p "$out/render"
            cd ${testSrc}/builder
            GFG_RENDER_OUT="$out/render" pytest -q
          '';
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell ((catalogInputs pkgs) // {
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
          # Tauri's Linux backend (GTK and WebKitGTK), to build and test the app on Linux.
          buildInputs = lib.optionals pkgs.stdenv.hostPlatform.isLinux (with pkgs; [
            atk
            cairo
            gdk-pixbuf
            glib
            gtk3
            librsvg
            libsoup_3
            pango
            webkitgtk_4_1
          ]);
        });
      });
    };
}
