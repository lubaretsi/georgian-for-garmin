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

      builderFiles = lib.fileset.unions [
        ./builder
        ./fonts/registry.json
        ./fonts/registry.schema.json
      ];
      src = lib.fileset.toSource { root = ./.; fileset = builderFiles; };
      # The builder tests also check the per-part size table in fonts/README.md and the watch
      # table in README.md.
      testSrc = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [ builderFiles ./fonts/README.md ./README.md ];
      };

      fontInputs = pkgs: {
        GFG_STOCK_DIR = pkgs.linkFarm "garmin-stock-fonts" (map
          (part: {
            name = part.file;
            path = pkgs.fetchurl { inherit (part) url sha256; };
          })
          stockParts);
        GFG_NOTO_SANS_GEORGIAN = pkgs.fetchurl { inherit (registry.georgian_font) url sha256; };
        GFG_NOTO_OFL = pkgs.fetchurl { inherit (registry.georgian_font.license) url sha256; };
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
          python -m garmin_fonts --registry ../fonts/registry.json --stock-dir "$GFG_STOCK_DIR" \
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
        builder = pkgs.runCommand "georgian-garmin-fonts-tests"
          ((fontInputs pkgs) // {
            nativeBuildInputs = [ (python pkgs) ];
            PYTHONDONTWRITEBYTECODE = 1;
          })
          ''
            cd ${testSrc}/builder
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
