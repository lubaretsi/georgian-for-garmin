# Font registry

`registry.json` lists every Garmin stock font pack that gets Georgian, the font the Georgian
glyphs come from, and the watches each pack was verified on. `registry.schema.json` defines its
format. `nix build .#fonts` downloads every part listed here by its SHA-256, adds Georgian to it
and writes `pack` and `packs` into the asset manifest. `nix flake check` validates the registry
against the schema, checks each stock file against its entry and rebuilds every font.

## Add a tested model

Add a model after installing a pack's Georgian build on that watch, unplugging and restarting
it, and seeing Georgian render in a notification. Append to the pack's `tested` list:

| Field        | Value                                                                                 |
| ------------ | ------------------------------------------------------------------------------------- |
| `model`      | `Model/Description` from the watch's `GARMIN/GarminDevice.xml`, e.g. `Venu 2`          |
| `model_part` | `Model/PartNumber`, e.g. `006-B3703-00`                                                |
| `software`   | `Model/SoftwareVersion` with two decimals: `1905` is `19.05`                            |
| `parts`      | The parts of this pack you saw rendering Georgian, e.g. `["006-D9035-03"]`             |

`georgian-for-garmin inspect` shows the model, part number and software version too. The same
model on a different software version is a separate entry.

## Add a pack

1. **Find the parts.** The watch's `GarminDevice.xml` lists its fonts as
   `MassStorageMode/UpdateFile` entries with `Path` `Garmin/ExtData`, a `PartNumber` such as
   `006-D9035-03` and a `Version` (`Major` 2, `Minor` 10 is `2.10`). Garmin's update catalog
   (what Garmin Express asks for that watch) names each part's product, release version,
   download URL, file size and MD5. A pack is one product in all its sizes, each a separate part.
2. **Download and hash.** Download each `.fnt` from its catalog URL, check its size and MD5
   against the catalog, then take its SHA-256: `shasum -a 256 006-D9035-03.fnt`.
3. **Fill in the entry.** Add to `packs`; the order matters (see [Pack order](#pack-order)):
   - `id`: lowercase words joined by hyphens, e.g. `armenian-noto-sans-regular`. Keep it
     stable once released; the asset manifest refers to it.
   - `name`: Garmin's product name as the catalog shows it, without the size, e.g.
     `Armenian Font, NotoSans Regular`.
   - `weight`: the weight the pack's stock glyphs are drawn at, as a value on Noto Sans
     Georgian's `wght` axis (100 to 900; 400 is Regular, 700 Bold). Georgian is drawn at it so it
     matches the stock letters. Go by the glyphs, not the product name (see [Weight](#weight)).
   - `parts`: one entry per size, smallest first, each with `part` (part number), `size_px`
     (nominal pixel size; Georgian is drawn at this size; see step 4 for reading it from the
     file), `version` (as in the catalog, `2.10`), `url` (the official
     `https://download.garmin.com/…/<part>.fnt` link) and `sha256` (64 lowercase hex digits).
     A part drawn at another weight than the rest of its pack gets its own `weight`.
   - `tested`: `[]`. A new pack is not marked tested until someone installs it on a watch and
     sees Georgian render (above).
4. **Check.** `git add fonts/registry.json`, then run `nix flake check`. It rejects a duplicate
   part or size, a malformed hash or weight, a tested part that isn't in the pack, a file whose
   footer names another part or version, and a size that doesn't fit the file's line height (a
   part's line height must be above its size and at most twice it, and must not shrink as size
   grows across the pack). To see a file's line height and baseline, from `builder/` in
   `nix develop`:

   ```sh
   python -c 'import sys; from garmin_fonts.codec import decode; h = decode(open(sys.argv[1], "rb").read())[0]; print("line height", h[7], "baseline", h[8])' 006-D9035-03.fnt
   ```

   In Garmin's Armenian packs the nominal size is about the height of a capital such as Ա, in
   pixels; the baseline's position varies from part to part.

   The build fails when a Georgian glyph doesn't fit a part's row. It drops at most two of the
   faintest pixels of a glyph and lists them under `clipped` in the asset manifest. Pin each new
   part's Georgian SHA-256, and any clipped pixels, in `builder/tests/test_build.py`.
5. **Install it on a watch** that lists one of the pack's parts, then add it as a tested model.

## Weight

Measure a pack's weight by drawing its letters with Noto Sans Armenian, at the size whose
capital height matches the stock file's, and finding the `wght` value whose total 2-bit ink
comes closest to the stock glyphs'. Garmin's "NotoSans Armenian Bold" product (v2.00) is
drawn at two weights:

| Parts                                               | Measured `wght` | Registry `weight`       |
| --------------------------------------------------- | --------------- | ----------------------- |
| `-17` 15 px, `-21` 21 px                            | 600 to 700      | 700 (the pack's)        |
| `-05` 13, `-06` 18, `-07` 20, `-08` 24, `-09` 25 px | 350 to 400      | 400 (per-part override) |

`-05` and `-06` have the same glyphs as the Regular product's `-18` (12 px) and `-20` (17 px).
The 400 overrides are deliberate: drawing Georgian Bold next to these Regular-weight letters
would make it stand out.

## Pack order

A watch gets the first pack, in registry order, that has a part the watch lists. Its other packs
keep their stock fonts. The order:

1. `armenian-noto-sans-regular` stays first; it is the pack tested on hardware (Venu 2).
2. `armenian-noto-sans-armenian-regular`: Regular faces are the likelier ones to carry message
   text, where Georgian matters most.
3. `armenian-noto-sans-armenian-bold`.

## Georgian source

`georgian_font` holds the TrueType font the glyphs are rendered from and its license, each with
a URL and SHA-256. Changing it changes every Georgian build, including the 27 px font verified
on hardware; the builder's tests pin those bytes.
