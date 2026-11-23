# Font registry

`registry.json` lists every Garmin stock font pack that gets Georgian, the host families those
packs belong to, other Garmin stock fonts pinned for experiments, the font the Georgian glyphs
come from, the watches each pack was verified on, and the support list: every watch model the
project knows of. `registry.schema.json` defines its format. `nix build .#fonts` downloads
every pack part listed here by its SHA-256, adds Georgian to it and writes `pack`, `packs` and
`watches` into the asset manifest. `nix flake check` validates the registry against the schema,
checks each stock file against its entry, rebuilds every font, round-trips every pinned stock
font through the codec, renders the test text over every watch report fixture
([Fallback models](#fallback-models)) and checks the README's watch table against the support
list.

A pack has a `kind`:

- `bitmap`: Garmin's 2-bit `.fnt` fonts, one part per pixel size. The installer stages them as
  `GARMIN/ExtData/<part>.tmp`, under the file name the watch declares (some watches declare
  Garmin's short form, `D894567.tmp`); the watch imports them when it restarts.
- `truetype`: Garmin's TrueType `.bin` fonts, one part per weight. The watch keeps them as
  `GARMIN/ExtData/<part>.bin`; the installer replaces that file (see
  [TrueType packs](#truetype-packs)).

## Host families

`hosts` lists the kinds of Garmin font Georgian is added to; every pack and `stock` entry names
one by `id`, of its own `kind`. A host gives:

| Field       | Value |
| ----------- | ----- |
| `script`    | The script its fonts are for: Armenian, Thai, Japanese, Korean, Chinese, Hebrew |
| `size_like` | What Georgian is sized by: `x_height` letters (and for bitmap hosts `capitals`, the tie-break), measured in each font itself ([Size](#size), [TrueType Size](#size-1)). Armenian: ա ո ս ռ ւ and Ա to Ֆ. Thai, which has no Latin: 28 Thai consonants with neither ascender nor descender, and the digit 0. Japanese, Korean and Chinese: their fonts' own Latin letters. TrueType hosts: Latin `oces`, else the em |
| `marker`    | A test build's activation marker in the host's own script (`glyph` drawn as `drawn_as`, e.g. ก as ข), so a photo shows a build is active even where the watch takes no Georgian from it |
| `confirmed_by` | Optional: ids of observations in `fallback-models.json`'s `evidence` where a watch drew Georgian from this family's fonts outside any pack's `tested` entries (a lab test) |

A pack installs only when its family is confirmed on a watch: some pack of that host has a
`tested` entry, or the host has `confirmed_by`. Every other pack must be `install: false`
(restore-only); the registry checks reject it otherwise. Korean is confirmed by the lab:
`venu-2-korean-host` (the Venu 2 drew Georgian from Garmin's Korean 27 px font with Georgian
added) and `venu-2-compressed-korean` (the same, re-encoded as Compressed). Its packs install
untested per pack, and the watches they reach stay predictions until a report confirms them.

The builder's `add_georgian` (`builder/garmin_fonts/build.py`) draws Georgian the same way for
every host; only the letters it measures differ. The builds of the Thai, Japanese, Korean and
Chinese fonts reproduce the lab's test builds byte for byte (`builder/tests/test_hosts.py`),
including the Thai 27 px build that drew Georgian on the Venu 2. Kosugi's 31 px (`006-D8944-23`)
and 17 px (`006-D8944-66`) rows are too short for Georgian at the matched size, and the build
says so.

## Other stock fonts

`stock` pins Garmin bitmap fonts that are in no pack: the Venu 2's Chinese, Japanese, Korean and
Thai fonts (`006-D8448-25..28`, `006-D8944-20..23`, `006-D8945-20/-21/-23`,
`006-D8947-20..23`) and the Lily 2 Active's "Compressed" Chinese and Japanese ones
(`006-D8448-70/-71`, `006-D8944-66/-67`). Each entry has its catalog `name`, `size_px`, `weight`
(500 Medium, 700 Bold), `host`, `version`, `url` and `sha256`. They are never built into the
app's fonts: a font family moves into `packs` only after a watch confirms it. `nix develop` and
the flake check fetch them by hash (`GFG_CATALOG_DIR`), and the builder's `catalog.fetch_stock`
finds them there, in its cache or by their pinned URL.

## Korean packs

Three packs carry Georgian in Garmin's Korean bitmap fonts, for watches that list no other font
this build installs, each part the version the watch declares and Garmin's catalog serves:

| Pack | Parts | For |
| ---- | ----- | --- |
| `korean-nanum-gothic-medium-compressed` | `006-D8945-67` 17 px, `-68` 22 px, 2.00, Compressed | Lily 2 Active (5.10) |
| `korean-nanum-gothic-medium` | `006-D8945-22` 27 px, 2.10 | Venu 1st gen (7.80) |
| `korean-nanum-gothic-bold` | `006-D8945-07` 19 px, 2.20 | Instinct 3 AMOLED 50mm and 45mm |

Which Korean size a notification uses is known only for the Venu 2, whose pop-up drew from the
27 px part (`fallback-models.json`). So the packs carry few sizes, to keep the app small: the
Venu 1st gen gets that same 27 px part, the Lily 2 Active both its sizes, and the Instinct 3 its
19 px text font, which both sizes declare; the large data-screen fonts ("DB … Size 30–69") are
left out. Garmin's catalog serves each part at one version: where a watch declares another, the
part is left out, since install would find an import it doesn't recognise and refuse. The
Instinct 3 declares `-11`, `-13`, `-15`, `-16` at
2.30 (`-09`, `-10`, `-12`, `-65` on the 45mm, at 2.30, 2.40 and 2.10) and `-63`, `-64` at 2.10,
where the catalog serves 2.20 and 2.00, and #33's imports of those differ in size from the
catalog's files; its `-07` matches in version and in size (1823701 bytes). The Venu 1st gen
declares its Korean fonts at 0.01, below any release, so `-22` is packed at the catalog's 2.10,
what Garmin Express would install. None of these packs has been installed on a watch.

"Compressed" fonts (header flags `0x0603` instead of `0x0403`) store each glyph as the
Connect IQ SDK's per-glyph RLE stream of its plain bitmap; the codec reads both and writes each
font back in its own format, byte for byte (`builder/garmin_fonts/codec.py`).

## Support list

`watches` is the one place a watch model's status comes from. Each model appears once:

| Field          | Value                                                                                  |
| -------------- | -------------------------------------------------------------------------------------- |
| `model`        | Its name for people, as the README table shows it, e.g. `Venu 2`; one part number Garmin sells under two names gets both, e.g. `fenix 7 Pro Solar, Pro Sapphire Solar` |
| `model_part`   | `Model/PartNumber` from the watch's `GARMIN/GarminDevice.xml`, e.g. `006-B3703-00`; unique |
| `issues`       | Its GitHub issue numbers, the main one first: the one its owners follow, where duplicates are closed into |
| `unsupported`  | Only for a model known not to work: `software`, the versions it was reported on, and `reason`, the id of an entry in `reasons` |
| `prediction`   | Only for a model not tested, predicted to work from Garmin's data or its own watch report ([Predicted models](#predicted-models)) |

`reasons` holds each reason once, by `id`, as `en` and `ka` sentences for the app (the fenix 6
models share `font_unused`). Every reason must be used.

The list holds only what can't be derived. A connected watch's status is worked out from the
list, the packs' `tested` entries and `install` settings and the fonts the watch lists
([How it works](../docs/how-it-works.md#the-watchs-font-folder)): `tested`, `expected`,
`unsupported` or `unknown`. So a new model with fonts this build knows needs no entry to install;
add one when it gets an issue, so the app links it. Add `unsupported` when a model is shown not to
work, and a version to its `software` when a report from a new version says the same; the app asks
an unsupported watch for a report only on a version that isn't listed.

The README's watch tables are generated from this list: after changing it, run
`python -m garmin_fonts.support` from `builder/` in `nix develop`. The builder tests fail when a
table and the list differ, and the registry checks reject a model listed twice, a tested model
missing from the list or listed as unsupported, an unsupported model without an issue, a reason
that is missing, repeated or unused, and a prediction on a tested or unsupported model.

### Predicted models

A `prediction` lists a model no one has tested yet whose fonts Garmin's own data, or the watch's
own report, says this project can add Georgian to, so the README can show it as likely to work
before anyone has confirmed it. Its fields:

| Field        | Value |
| ------------ | ----- |
| `family`     | `noto_truetype`: Garmin's [Connect IQ device reference](https://developer.garmin.com/connect-iq/articles/device-reference/) lists Noto Sans Armenian (`NotoSansArmenian-Bold`/`-Regular`, Garmin's `006-D9971-08/-09`) among the model's scalable fonts, the fonts Georgian works through on the Forerunner 165. `bitmap_armenian`: the model gets the same firmware as a tested model with bitmap Armenian (`006-D9035`); today only the fenix 7 family (fenix 7 / 7S / 7X and their Pro, epix (Gen 2) and Pro, MARQ (Gen 2), D2 Mach 1), whose 10.43 notes add Armenian characters. `bitmap_korean`: the model's own report declares Garmin's Korean bitmap fonts (`006-D8945`) and no Armenian font; on the maintainer's Venu 2 (19.05), Georgian added to the Korean 27 px font `006-D8945-22`, plain and re-encoded as Compressed, rendered natively in notifications. Today the Venu (1st gen), both Instinct 3 AMOLED and the Lily 2 Active |
| `confidence` | `high`, the only value allowed: a weaker guess stays out of the list |
| `basis`      | Why, in a sentence. When one device page gives one name to several part numbers, `model` is that page's name and `basis` says so; don't guess which part is which |
| `source`     | The device reference page, `firmware family of <model>` naming a tested model in this list, the GitHub issue whose watch report declares the font, or `a watch report emailed to the maintainer` (the report's fixture is in `crates/core/tests/reports`) |

A prediction changes nothing on a connected watch: the app still decides from the fonts the watch
lists, so a watch that lists none of this build's fonts is `unknown` and asked for a report.
`inspect` and the report show the prediction (`support … predicted=noto_truetype`), so a report
from a predicted model says at a glance whether the prediction held.

The Connect IQ pages leave out fonts a watch has (the Venu 3's page lacks
`006-D9971-09`), so a page without Noto Sans Armenian is no evidence against a model; such models
stay out. When a predicted model gets a report: confirmed, drop `prediction` and add the tested
entry and its issue; shown not to work, replace `prediction` with `unsupported`. The registry
checks reject a model that is predicted and tested, or predicted and unsupported. The 2026-11-22
entries come from every device reference page, with the rules checked against the 17 reported
model parts first: none disagreed.

## Add a tested model

Add a model once someone has installed a pack's Georgian build on that watch, unplugged and
restarted it, and confirmed Georgian in a real notification, with a photo or a clear report in a
GitHub issue. The owner of a watch rarely knows which font size a view uses, so the entry covers
the model and software version. List every part of the pack the app installed on it (the parts
the watch lists). The model must be in the [support list](#support-list), with the issue. Append
to the pack's `tested` list:

| Field        | Value                                                                                 |
| ------------ | ------------------------------------------------------------------------------------- |
| `model_part` | `Model/PartNumber`, e.g. `006-B3703-00`                                                |
| `software`   | `Model/SoftwareVersion` with two decimals: `1905` is `19.05`                            |
| `parts`      | The parts of this pack installed on that watch, e.g. `["006-D9035-03"]`                 |

`georgian-for-garmin inspect` shows the model, part number and software version too. The same
model on a different software version is a separate entry. Then regenerate the README's watch
table.

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
   - `host`: the pack's host family in `hosts` ([Host families](#host-families)), e.g.
     `armenian`; it decides what Georgian is sized by.
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
- **Armenian x-height**: the median height of ա ո ս ռ ւ. **Capitals**: the median of Ա to Ֆ
  (the `armenian` host's `size_like`; other hosts measure their own letters).
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

The Korean packs are sized by their fonts' own Latin letters (x-height `acemnorsuvwxz`, capitals
for the tie-break), the same rule:

| Part           | Nominal | Latin x-height | Latin capitals | Georgian font size | Descenders | Mkhedruli x-height | Mtavruli height |
| -------------- | ------- | -------------- | -------------- | ------------------ | ---------- | ------------------ | --------------- |
| `006-D8945-67` |   17 px |           11.7 |           15.3 |              21 px |        81% |               12.0 |            16.0 |
| `006-D8945-68` |   22 px |           15.0 |           20.0 |              27 px |        79% |               15.0 |            20.0 |
| `006-D8945-22` |   27 px |           17.7 |           23.3 |              32 px |        65% |               18.0 |            23.0 |
| `006-D8945-07` |   19 px |           13.0 |           17.3 |              23 px |        93% |               13.0 |            17.0 |

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

### Version trailer

Every Garmin `.bin` ends with 16 bytes after the masked font: raw (unmasked) `0x89`, the release
version as a little-endian u16 (`200` is 2.00, `210` is 2.10), the part number in ASCII and a
NUL; the table data before it is padded with masked zero bytes (`0x76`) to a 4-byte boundary.
Stock `006-D0952-08` ends `89 d2 00 "006-D0952-08" 00`. Watches read the version there; a file
without it is listed in `GarminDevice.xml` below 1.00 (0.01 on the Forerunner 165, #5; 0.00 on
the fenix 6S Pro, #6). Each build pads the same way and keeps the stock trailer byte for byte,
and the builder checks every stock trailer against the registry's part and version. Builds up to
0.3.0-beta.2 left it out. Install and restore identify a file by its content; only where they
can't read it does the declared version decide, and then only restore accepts a version below
1.00 ([How it works](../docs/how-it-works.md#truetype-fonts)).

### Which builds install a pack

A pack's `install` decides: `true` (the default) or `false`, restore-only. Pre-release builds
install exactly what stable builds do: a beta never installs a font family no watch has
confirmed ([Host families](#host-families)). Swiss 721 Hebrew (`hebrew-swiss-721-truetype`) is
`false`: on a fenix 6S Pro our font reached the watch (the hidden listing showed
`006-D0952-07/-08.bin ro georgian_beta`), its Georgian Unicode range bit was set, and the watch
still drew question marks (#6). Every build still restores a pack it doesn't install, whatever
host install chooses on that watch, and recognizes its Georgian builds, so a beta tester can put
Garmin's originals back.

### Earlier builds

A part's `earlier_georgian` lists, by SHA-256 and size, the Georgian builds earlier releases
installed: 0.3.0-beta.2's marked builds, and for TrueType parts 0.3.0-beta.1's (both without
the trailer). `inspect` reports such a file as `georgian_beta`; install and restore replace it.
The builder tests pin the marked builds to the beta.2 entries (TrueType ones with the trailer
removed) and the stable TrueType builds, without their trailer, to the beta.1 entries.

### Hosts

A watch gets Georgian in one family of fonts: the first installable pack in registry order that
it lists chooses ([Pack order](#pack-order)). Bitmap Armenian comes first: a watch that lists
any of it gets Georgian there, exactly as before TrueType support, and its TrueType and Korean
fonts stay Garmin's (the fenix 7 Pro keeps its install). TrueType comes next, all its installable
packs together; Korean last. `inspect` reports the choice under `hosts`, with the reason and the
parts it leaves alone.

A watch without bitmap Armenian parts gets Georgian in every installable TrueType part below
that it lists. Which font a watch's firmware draws a Georgian letter from is not known for these
watches: the
Venu 2 drew one from a Thai bitmap pack, so it searches loaded fonts beyond the one for the
text's script, but no TrueType watch has been tested. More than one host per watch raises the
chance that the font the firmware searches has Georgian.

| Watches (reported software)                      | TrueType fonts they list                               | Get Georgian in                                     |
| ------------------------------------------------ | ------------------------------------------------------ | --------------------------------------------------- |
| Venu 3, Venu 3S (17.05)                          | `006-D9971-09`, `006-D1053-96/-98`, CJK, Thai          | `006-D9971-09`, `006-D1053-98`                      |
| Forerunner 165, 165 Music (29.52)                | `006-D9971-08/-09`, `006-D1053-96..99`, CJK, Thai      | `006-D9971-08/-09`, `006-D1053-98/-99`              |
| fenix 6 Pro Solar, 6S Pro, 6X Pro (28.02)        | `006-D0952-05..08`                                     | none; `006-D0952-07/-08` restore only               |
| Instinct 3 AMOLED 50mm (15.19), 45mm (16.12)     | `006-D0952-05..08`, `006-D9971-00..03`                 | none; `006-D0952-07/-08` restore only               |
| fenix 7 Pro Solar (26.09), also bitmap Armenian  | `006-D0952-05..08`, `006-D9971-00..03`                 | its bitmap Armenian parts only                      |

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
  Garmin's fonts modified. Restore-only: the fenix 6S Pro didn't use it for Georgian (#6).

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
timestamp changes, so the same inputs give the same bytes. The builder tests compare every build
with its stock original: the same table list, header and trailer, every other table byte for
byte, every stock glyph's outline, metrics and cmap entry unchanged. fontTools writes the table
data in its own order, so in the Noto Sans Armenian and Swiss 721 Hebrew builds the tables'
data sits in a different order than in Garmin's file (the table list itself is unchanged); the
tests record that as an expected difference, and the hashes pinned for the builds include it.

## Pack order

Registry order chooses a watch's font family: the first installable pack it lists decides
([Hosts](#hosts)). It also sets the order of packs in `inspect` and install output, and the
order files are written in:

1. `armenian-noto-sans-regular`, the pack tested on hardware (Venu 2).
2. `armenian-noto-sans-armenian-regular`.
3. `armenian-noto-sans-armenian-bold`.
4. `armenian-noto-sans-armenian-truetype`, the first TrueType pack: the closest match to the
   bitmap Armenian packs.
5. `hebrew-noto-sans-hebrew-truetype`.
6. `hebrew-swiss-721-truetype`, restore-only.
7. `korean-nanum-gothic-medium-compressed`, `korean-nanum-gothic-medium`,
   `korean-nanum-gothic-bold`: last, for watches that list none of the above.

## Test marks

Pre-release builds (a Cargo workspace version with a `-`, such as `0.3.0-beta.3`) mark each
part, so a photo of the test notification `Test მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ Test` tells which part the watch
rendered. Each part's Georgian draws a solid bar under, over or on both sides of one capital of
`ᲛᲗᲐᲕᲠᲣᲚᲘ`; every other glyph is the release build's. Variants are assigned per kind, in
registry order (packs in order, parts in pack order): part i, counting from 0, is variant `A` + i,
marks capital i mod 8, and draws its bar below for parts 0 to 7, above for 8 to 15 and on both
sides from 16. Adding or moving a part changes the variants after it.

`nix build .#fonts` builds with marks exactly when the version is a pre-release;
`.#fonts-stable` never does and `.#fonts-marked` always does (`python -m garmin_fonts build
--test-marks`). A marked build sets `test_marks` in the manifest and gives each font a `mark`:
its variant, marked code point, position and style, and a picture of the marked capitals,
`previews/mark-<part>.png`, with its SHA-256. `python -m garmin_fonts.preview --marks <fonts dir>
--out marks.png` draws all of them on one sheet.

## Fallback models

`fallback-models.json` (format: `fallback-models.schema.json`) says, per watch view, which font
draws a character, as far as recorded observations show; the builder's `render` uses it. A
model applies to the watch model parts and software it was observed on, and only to the code
points the watch's built-in font was seen to lack (`system_lacks`). For those it lists the
ExtData parts the view takes them from, first match first (`order`), parts seen to supply them
when no other font had them, whose place in the order is unknown (`sole`), the parts it was seen
not to use (`unused`: other sizes), the Garmin originals present while such characters drew as
boxes (`stock_without_effect`, by part and version), and what backs drawing a box (`missing`).
Every step cites entries of `evidence`: what was seen, and where it is recorded (paths in this
repository, which the tests check exist, or lab records by name and section). `caveats` keeps
observations a model doesn't explain.

`render` marks each character `confirmed` (every input known and the step that picks the font
backed by cited evidence), `coverage_only` (a font the watch has includes the glyph, but the
model doesn't place that font, or places it only as the sole supplier while another font has
the glyph too), or `unknown` (a font's bytes are unavailable, the watch's built-in font may
draw it, or no model fits). A report fixture lists the declared files, not the renderer's order,
so it never confirms an order on its own. A font given as bytes counts as Garmin's original only
when it is the registry's pinned file.

The one model so far is the Venu 2's notification pop-up (19.05): Georgian comes from the Thai
27 px font (`006-D8947-22`) before the Armenian one (`006-D9035-03`); the Korean 27 px font
(`006-D8945-22`, plain or Compressed) supplies it when it is the only one with Georgian; the
other sizes aren't used. Add a model or step only with an observation to cite; anything else
stays `unknown` or `coverage_only`.

## Georgian source

`georgian_font` holds the TrueType font the glyphs are rendered from and its license, each with
a URL and SHA-256. Changing it changes every Georgian build, including the parts tested on a
watch, and may change the sizes in the table above; the builder's tests pin those bytes.

Its `rasterizer` names the library versions that draw the glyphs (Pillow, FreeType, raqm,
fontTools), as in `nix develop`: other versions, or Pillow without raqm, give other bytes for the
same inputs. A builder test checks that the environment matches it, so a nixpkgs update that
changes them fails until the new builds are tested on a watch and the pins updated together;
builds made elsewhere are marked `rasterizer_pinned: false`.
