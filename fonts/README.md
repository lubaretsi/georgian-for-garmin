# Font registry

`registry.json` lists every Garmin stock font pack that gets Georgian, the font the Georgian
glyphs come from, and the watches each pack was verified on. `registry.schema.json` defines its
format. `nix build .#fonts` downloads every part listed here by its SHA-256, adds Georgian to it
and writes `pack` and `packs` into the asset manifest. `nix flake check` validates the registry
against the schema, checks each stock file against its entry and rebuilds every font.

A pack has a `kind`:

- `bitmap`: Garmin's 2-bit `.fnt` fonts, one part per pixel size. The installer stages them as
  `GARMIN/ExtData/<part>.tmp`; the watch imports them when it restarts.
- `truetype`: Garmin's TrueType `.bin` fonts, one part per weight. The watch keeps them as
  `GARMIN/ExtData/<part>.bin`; the installer replaces that file (see
  [TrueType packs](#truetype-packs)).

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
   - `kind`: `bitmap` for `.fnt` parts, `truetype` for `.bin` parts.
   - `parts`: for a bitmap pack, one entry per size, smallest first, each with `part` (part
     number), `size_px` (nominal pixel size; see step 4 for reading it from the file), `version`
     (as in the catalog, `2.10`), `url` (the official `https://download.garmin.com/…/<part>.fnt`
     link) and `sha256` (64 lowercase hex digits). For a TrueType pack, one entry per weight,
     Regular first, with `part`, `version`, `url` (`…/<part>.bin`) and `sha256`, and no size.
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
   pixels; the baseline's position varies from part to part. Georgian is not drawn at this
   size but measured against the stock letters (see [Size](#size)).

   The build fails when a Georgian glyph doesn't fit a part's row. It drops at most two of the
   faintest pixels of a glyph and lists them under `clipped` in the asset manifest. Pin each new
   part's Georgian SHA-256, and any clipped pixels, in `builder/tests/test_build.py`, and add
   its row to the table under [Size](#size); the builder tests print the row when it's missing.
   A TrueType part's Georgian SHA-256 goes in `builder/tests/test_truetype.py` and its row in
   the table under [TrueType packs](#truetype-packs).
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

## Size

A part's nominal size is about the height of an Armenian capital, not a font size: Georgian
drawn with Noto Sans Georgian at the nominal size came out about a quarter smaller than the
letters around it. The builder (`builder/garmin_fonts/georgian.py`) measures instead, on each
stock file's own 2-bit bitmaps:

- **Letter height**: each row of a glyph counts as its darkest pixel's level, a full-strength
  row being 1 px, so antialiased top and bottom rows count by coverage.
- **Armenian x-height**: the median height of ա ո ս ռ ւ. **Capitals**: the median of Ա to Ֆ.
- **Font size**: Noto Sans Georgian, at the part's weight, at the size from 1 px to twice the
  nominal size whose Mkhedruli x-height (median of ა თ ი ო) is closest to the Armenian
  x-height. Ties go to the Mtavruli height closest to the Armenian capitals, then to the smaller
  size. Hinting rounds each height to whole pixels, so one size can't match both: Mkhedruli
  comes within 1 px of the Armenian x-height in every part (a builder test checks it), and
  Mtavruli up to 2 px below the Armenian capitals.
- **Descenders**: Noto's Georgian descenders (ქ ფ ყ ჟ ღ ჯ ჩ and others) reach 25% of the font
  size below the baseline, deeper than the Armenian ones the rows were made for. At the matched
  size 14 of the 20 parts would need one or two more rows below the baseline (`-03`: 9, it has
  7). The builder keeps the size and the baseline and shortens the part of each outline more
  than 10 font units (1% of the font size) below the baseline, by whole percent, until every
  glyph fits under the clipping rule above; round letters' overshoot stays. Letters without
  descenders, ascenders and Mtavruli are drawn as designed.

Drawing at the largest size at which Noto's outlines fit unchanged was the alternative: the
descenders limit it too, leaving `-03`'s Mkhedruli x-height at 16 px against the Armenian 19.

"Descenders" is the length kept below the overshoot, in percent of Noto's. The asset manifest
lists each part's `georgian_size_px` and `georgian_descender_percent`.

| Part           | Nominal | Armenian x-height | Armenian capitals | Georgian font size | Descenders | Mkhedruli x-height | Mtavruli height |
| -------------- | ------- | ----------------- | ----------------- | ------------------ | ---------- | ------------------ | --------------- |
| `006-D9035-00` |   19 px |              13.0 |              18.0 |              23 px |        92% |               13.0 |            17.0 |
| `006-D9035-01` |   21 px |              14.7 |              20.0 |              27 px |        94% |               15.0 |            20.0 |
| `006-D9035-02` |   24 px |              17.0 |              23.0 |              30 px |        82% |               17.0 |            22.0 |
| `006-D9035-03` |   27 px |              19.0 |              26.0 |              34 px |        85% |               19.0 |            25.0 |
| `006-D9035-04` |   31 px |              22.0 |              30.0 |              40 px |        94% |               22.0 |            29.0 |
| `006-D9035-10` |   11 px |               7.0 |               9.3 |              13 px |       100% |                6.7 |             8.7 |
| `006-D9035-18` |   12 px |               8.3 |              11.7 |              15 px |        95% |                8.0 |            10.0 |
| `006-D9035-19` |   15 px |              10.7 |              14.7 |              19 px |       100% |               11.0 |            14.0 |
| `006-D9035-20` |   17 px |              12.0 |              16.7 |              21 px |       100% |               12.0 |            16.0 |
| `006-D9035-11` |   25 px |              18.0 |              24.7 |              32 px |        90% |               18.0 |            24.0 |
| `006-D9035-12` |   28 px |              19.3 |              27.0 |              34 px |        97% |               19.0 |            25.0 |
| `006-D9035-13` |   32 px |              22.3 |              30.7 |              40 px |        94% |               22.0 |            29.0 |
| `006-D9035-14` |   37 px |              26.3 |              36.0 |              47 px |        96% |               26.0 |            34.0 |
| `006-D9035-05` |   13 px |               8.3 |              11.7 |              15 px |        95% |                8.0 |            10.0 |
| `006-D9035-17` |   15 px |              10.0 |              14.0 |              17 px |       100% |               10.0 |            13.0 |
| `006-D9035-06` |   18 px |              12.0 |              16.7 |              21 px |       100% |               12.0 |            16.0 |
| `006-D9035-07` |   20 px |              14.0 |              19.0 |              25 px |        85% |               14.0 |            18.0 |
| `006-D9035-21` |   21 px |              14.7 |              20.0 |              26 px |        96% |               15.0 |            19.0 |
| `006-D9035-08` |   24 px |              16.3 |              22.3 |              29 px |       100% |               16.0 |            21.0 |
| `006-D9035-09` |   25 px |              18.0 |              24.7 |              32 px |        90% |               18.0 |            24.0 |

## TrueType packs

Many newer watches list no bitmap Armenian pack but do list TrueType fonts (watch reports in
GitHub issues #2 to #14). Garmin's `.bin` fonts are plain TrueType files with every byte
XOR-ed with `0x76`. The builder (`builder/garmin_fonts/truetype.py`) unmasks Garmin's file,
appends Noto Sans Georgian's Mkhedruli (U+10D0 to U+10F0) and Mtavruli (U+1C90 to U+1CB0),
at the part's weight, and masks it again. Every glyph, table, name and metric of the host stays;
GSUB, GPOS, hinting programs and names are copied byte for byte. The Georgian glyphs are
unhinted, as in Garmin's own Noto Sans Georgian (`006-D9971-39`), whose Regular outlines are
identical to the ones the builder takes from the variable font at weight 400. The build writes
a preview of each part, `previews/<part>.png`, showing the host's own text beside
`მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ`.

### Hosts

A watch gets Georgian in every TrueType part below that it lists, as with bitmap packs. Which
font a watch's firmware draws a Georgian letter from is not known for these watches: the
Venu 2 drew one from a Thai bitmap pack, so it searches loaded fonts beyond the one for the
text's script, but no TrueType watch has been tested. More than one host per watch raises the
chance that the font the firmware searches has Georgian.

| Watches (reported software)                      | TrueType fonts they list                               | Get Georgian in                                     |
| ------------------------------------------------ | ------------------------------------------------------ | --------------------------------------------------- |
| Venu 3, Venu 3S (17.05)                          | `006-D9971-09`, `006-D1053-96/-98`, CJK, Thai          | `006-D9971-09`, `006-D1053-98`                      |
| Forerunner 165, 165 Music (29.52)                | `006-D9971-08/-09`, `006-D1053-96..99`, CJK, Thai      | `006-D9971-08/-09`, `006-D1053-98/-99`              |
| fenix 6 Pro Solar, 6S Pro, 6X Pro (28.02)        | `006-D0952-05..08`                                     | `006-D0952-07/-08`                                  |
| Instinct 3 AMOLED 50mm (15.19)                   | `006-D0952-05..08`, `006-D9971-00..03`                 | `006-D0952-07/-08`                                  |
| fenix 7 Pro Solar (26.09), also bitmap Armenian  | `006-D0952-05..08`, `006-D9971-00..03`                 | its bitmap Armenian parts and `006-D0952-07/-08`    |

- **Noto Sans Armenian** (`armenian-noto-sans-armenian-truetype`: `006-D9971-09` Regular,
  `-08` Bold): the same design family as Noto Sans Georgian, with the same units per em and
  vertical metrics, 30 KB, and a script few people in Georgia read. Listed by the Venu 3 and
  Forerunner 165.
- **Noto Sans Hebrew** (`hebrew-noto-sans-hebrew-truetype`: `006-D1053-98` Regular, `-99`
  Bold): a second Noto host on the same watches, 27 KB, so Georgian is in two font families
  there. Without it the Venu 3 would have one host only.
- **Swiss 721 Hebrew** (`hebrew-swiss-721-truetype`: `006-D0952-08` Roman, `-07` Bold): the
  only small fonts the fenix 6 and Instinct 3 list that are neither Arabic nor CJK; 38 KB, with
  Latin letters to size Georgian by. A Bitstream font; the project owner allows shipping
  Garmin's fonts modified.

Not used: the Arabic fonts (`006-D0952-05/-06` Sakkal Majalla, `006-D1053-96/-97` Noto Naskh
Arabic) depend on shaping tables a modification could disturb, are 250 to 355 KB, and every
watch that lists them also lists a Hebrew host. The CJK and Thai fonts are 0.2 to 11 MB, so
replacing them over MTP is slow, and their readers depend on them. Garmin's own Noto Sans
Georgian (`006-D9971-39`) is listed by none of the reporting watches.

### Size

Georgian is scaled from Noto's 1000 units per em into the host's units, by `choose_sizing`:

- **Host with Latin letters** (Swiss 721 Hebrew): Mkhedruli x-height matches the host's Latin
  x-height. Both are the median top of round letters, which include their overshoot: ა თ ი ო
  against o c e s.
- **Host without Latin** (Noto Sans Armenian, Noto Sans Hebrew): Noto's own proportions, the
  host's units per em over Noto's; Noto fonts are designed to be set together at one size.
- **Limits**: Georgian stays within the tighter of the host's `hhea` and OS/2 Windows ascent
  and descent, which renderers clip to. If the tallest Georgian letter would rise above the
  ascent, the scale shrinks to fit (no current host needs it). Descenders below the overshoot
  are shortened, by whole percent, as in the bitmap builds, until they stay within the descent:
  Swiss 721 Hebrew's 483-unit descent takes 95%. OS/2 typo metrics set line spacing, and the
  hosts' own glyphs reach past them, so they don't limit Georgian.

The asset manifest lists each part's `georgian_scale` (an exact fraction), its
`georgian_scale_basis` and `georgian_descender_percent`. Heights are in host font units:

| Part           | Font                       | Units/em | Matched to     | Scale  | Descenders | Host Latin x-height | Mkhedruli x-height | Georgian top, bottom | Host ascent, descent |
| -------------- | -------------------------- | -------- | -------------- | ------ | ---------- | ------------------- | ------------------ | -------------------- | -------------------- |
| `006-D9971-09` | Noto Sans Armenian Regular |     1000 | em             | 1.0000 |       100% |                none |                546 |            770, -250 |           1069, -293 |
| `006-D9971-08` | Noto Sans Armenian Bold    |     1000 | em             | 1.0000 |       100% |                none |                556 |            770, -250 |           1069, -293 |
| `006-D1053-98` | Noto Sans Hebrew           |     2048 | em             | 2.0480 |       100% |                none |               1118 |           1577, -512 |           2189, -600 |
| `006-D1053-99` | Noto Sans Hebrew Bold      |     2048 | em             | 2.0480 |       100% |                none |               1139 |           1577, -512 |           2189, -600 |
| `006-D0952-08` | Swis721Hebrew BT Roman     |     2048 | Latin x-height | 2.0256 |        95% |                1106 |               1106 |           1560, -482 |           1565, -483 |
| `006-D0952-07` | Swis721Hebrew BT Bold      |     2048 | Latin x-height | 2.0252 |        95% |                1126 |               1126 |           1559, -482 |           1587, -483 |

Tables the build updates for the new glyphs: `cmap` (each Unicode subtable of format 4 or 12;
Swiss 721 Hebrew's Mac Roman subtable can't hold Georgian), `glyf`, `loca`, `hmtx`, `post`
glyph names (`uniXXXX`), `maxp` and `hhea` totals, the `head` bounding box, OS/2's Georgian
Unicode range bit, and in Swiss 721 Hebrew `hdmx` (linear widths), `LTSH` and `VDMX`. No
timestamp changes, so the same inputs give the same bytes.

## Pack order

A watch gets Georgian in every part, of every pack, that it lists. Registry order only sets the
order of packs in `inspect` and install output, and the order files are written in:

1. `armenian-noto-sans-regular`, the pack tested on hardware (Venu 2).
2. `armenian-noto-sans-armenian-regular`.
3. `armenian-noto-sans-armenian-bold`.
4. `armenian-noto-sans-armenian-truetype`, the first TrueType pack: the closest match to the
   bitmap Armenian packs.
5. `hebrew-noto-sans-hebrew-truetype`.
6. `hebrew-swiss-721-truetype`.

## Georgian source

`georgian_font` holds the TrueType font the glyphs are rendered from and its license, each with
a URL and SHA-256. Changing it changes every Georgian build, including the parts tested on a
watch, and may change the sizes in the table above; the builder's tests pin those bytes.
