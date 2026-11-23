# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The redesigned app UI's first phase, for developers: `crates/app/frontend` (React, TypeScript,
  Vite) with Storybook, where every screen and the Connect → Install → Restart → Check flows run
  over a simulated watch built from the core's real results
  ([development](docs/development.md#frontend-and-storybook)). The shipped app is unchanged.
  `nix develop` has Node and pnpm; `crates/core/tests/ui_fixtures.rs` writes and checks the
  fixtures; CI generates the core's TypeScript types, typechecks, and runs the flow and story
  tests in Chromium and WebKit with axe.

## [0.3.4] - 2026-11-24

### Added

- The font builder is a toolkit for font and watch experiments (`garmin_fonts`, from `builder/`;
  [development](docs/development.md#font-toolkit)): the codec reads and writes Garmin's
  "Compressed" bitmap fonts (per-glyph RLE, as the Lily 2 Active's Chinese, Japanese and Korean
  fonts are) byte for byte; `catalog.fetch_stock(part)` fetches stock fonts, pinned ones by
  SHA-256, others from Garmin's update catalog, cached, offline in CI; `add_georgian(font,
  size_like=…, markers=…)` adds Georgian to any host family's font; `render` draws text as a
  watch would, each character `confirmed`, `coverage_only` or `unknown` from cited fallback models
  (`fonts/fallback-models.json`); `Watch()` drives the watch through `georgian-for-garmin rpc`.
  The builder's command line is `python -m garmin_fonts fetch|build|render|roundtrip`, with
  `--json`, `--input` and `--help --json`; the asset build is now `python -m garmin_fonts build`.
- The font registry lists host families (`hosts`: Armenian, Thai, Japanese, Korean, Chinese and
  the TrueType hosts, each with the letters Georgian is sized by and a test marker) and pins the
  Venu 2's and Lily 2 Active's Chinese, Japanese, Korean and Thai fonts (`stock`), which the app's
  fonts don't include. The app's fonts are byte for byte the same.
- `nix flake check` round-trips every pinned stock font and renders the test notification over
  every watch report fixture; CI uploads the PNGs and per-character states as `render-report`.
  CI also checks every request the `Watch` client builds against the input schemas a dev build
  of the CLI declares, that every operation has a method, and the client against the real
  `rpc`.
- `nix run .#omp-venv` writes a `.venv` whose Python is the builder's pinned one with the
  toolkit on its path, so omp's eval kernel can use `garmin_fonts` directly
  ([development](docs/development.md#using-the-toolkit-from-omps-eval)). Builds record the
  rasterizer that drew them (Pillow, FreeType, raqm, fontTools) against the pinned one in the
  registry (`georgian_font.rasterizer`): Pillow without raqm, as from PyPI, gives other bytes, so
  `add_georgian` warns there and `Watch.stage` refuses such a build unless asked.
- When no watch has turned up 10 seconds after the waiting screen comes up, it shows a short
  help card under the spinner: quit Garmin Express, which can keep the app from seeing the watch
  (from its menu-bar icon on macOS, its notification-area icon on Windows; on Linux, eject the
  watch in the file manager instead), unplug and reconnect the watch choosing MTP, and use a data
  cable. It goes away as soon as a watch is found.
- The demo build's `GEORGIAN_FOR_GARMIN_DEMO_WATCH=none` never finds a watch, and
  `GEORGIAN_FOR_GARMIN_DEMO_PLATFORM` shows another platform's words.
- When another program holds the watch (`device_busy`), the app names it when the system tells
  and gives that program's steps for this system only: Garmin Express (quit it from its menu-bar
  or notification-area icon, or in Activity Monitor or Task Manager), macOS's camera service
  `ptpcamerad` (quit photo apps and reconnect, then share it on GitHub if it's still busy), the
  desktop's file access on Linux (eject the watch), another named program (quit it), or an
  unknown one (restart the computer as a test, look in Activity Monitor or Task Manager). It no
  longer shows Linux steps on macOS or Windows. **Share on GitHub** on that screen sends what the
  app found.
- `device_busy` errors carry `busy`: the holder (`kind`: `garmin_express`,
  `macos_camera_service`, `desktop_mtp`, `other` or `unknown`; process `name`, `pid`,
  `confirmed`), the watch's USB `product_id` and `product_name`, and its MTP `interface` class.
  macOS reads the IOKit registry's `UsbExclusiveOwner`, Linux `/proc`, Windows only whether Garmin
  Express is running. Nothing is stopped or signalled. The CLI's message names the holder too.
- `issue_link` takes `busy`: alone (no `report`) it makes a report with the `app` and `busy`
  lines, titled `Busy watch: …`; with a report, the `busy` line follows `connection`.
  `GEORGIAN_FOR_GARMIN_DEMO_BUSY` shows each case in the demo build.
- The epix Pro (Gen 2) 51mm (`006-B4314-00`) is tested on software 27.18
  ([#46](https://github.com/lubaretsi/georgian-for-garmin/issues/46)): with the bitmap Armenian
  fonts NotoSans Armenian Regular and Bold, Georgian showed in the pop-up, the notification list
  and the opened notification. It was the first predicted model a report confirmed; the two other
  parts under that Connect IQ name stay predicted.
- `georgian-for-garmin rpc` (JSON-RPC 2.0, one message per line) and `georgian-for-garmin mcp`
  (MCP over stdio, with the official Rust SDK) run every operation in one long-lived session that
  keeps the watch open between calls and opens it again after an unplug and replug (`no_device`
  meanwhile). `install`, `restore_stock`, `stage_file` and `wait_for_watch` send
  `notifications/progress` for a `_meta.progressToken` and stop on `notifications/cancelled`; a
  cancelled install or restore tries to undo what it wrote, as a failed one does. A line that
  isn't JSON gets -32700 and the session goes on.
- `extdata-list` (`extdata_list`) lists every file in `GARMIN/ExtData`, including the read-only
  fonts the watch imported, which only Garmin's `GetObjectIds` shows, with size, read-only flag
  and, with `--hash`, SHA-256s. It pages, or writes the whole list to a file.
- `wait-for-watch` (`wait_for_watch`) waits until a watch can be opened, as after a restart.
- Development builds (cargo feature `dev`, never in releases) add `read-file`, `stage-file` and
  `remove-staged` for font experiments: stage any file under a part the watch declares (also by
  short name, as `D844870.tmp`), with install's checks and readback, and remove it later by the
  id it logged.
- Development builds also add raw MTP operations for debugging from a script over `rpc` or
  `mcp` (and as CLI subcommands): `mtp_device_info`, `mtp_storage_ids`, `mtp_storage_info`,
  `mtp_handles` (with `hidden`, Garmin's GetObjectIds), `mtp_object_info`, `mtp_read` (whole or
  a range, as base64 or to a file), `ptp_transaction` (reads only, from a fixed list; USB only),
  and `mtp_trace`, which sends every PTP transaction of a session as `notifications/trace`. The
  only writes, `mtp_create` and `mtp_delete`, need `allow_write` and make `stage_file`'s checks
  with no override: only in `GARMIN/ExtData`, only over files identified as Garmin's, this
  program's or this tool's, never a hidden import; each is logged in `staged.jsonl`, and
  `remove_staged` removes what `mtp_create` wrote. Identifiers are masked unless
  `reveal_identifiers`: the serial number in `mtp_device_info` and a raw GetDeviceInfo, the
  owner's names in the 0xD401/0xD402 device properties, and the `<Id>` (unit ID), `<Ifix>` and
  `<AppId>` values in `GarminDevice.xml` (also in `read_file`). The Python `Watch` client has
  them as `watch.mtp.*`, `watch.ptp(opcode, params)` and `with watch.trace() as transactions:`.
- Error code `cancelled`, for a call cancelled in `rpc` or `mcp`.
- Errors after a failed or cancelled write carry `rollback`: `complete` or `incomplete`, with each
  file it tried to delete or put back and whether that worked. An incomplete rollback now says
  so plainly, names the files that may be left, and points to Restore original font
  (`restore-stock`); the app shows it in both languages. Before, the message said "Rolled back"
  even when a step had failed.
- The Lily 2 Active (`006-B4477-00`, 5.10) in the support list, predicted to work through
  Garmin's Korean bitmap fonts (`prediction` family `bitmap_korean`): its report, emailed to the
  maintainer, declares Korean `006-D8945-67/-68` (Compressed, 17 and 22 px) under short names,
  and on the maintainer's Venu 2 (19.05) Georgian added to Garmin's Korean 27 px font
  (`006-D8945-22`), plain and re-encoded as Compressed, showed in notifications. Its report is a
  replay fixture (`email-lily2active.json`, owner anonymized). Install and restore on it are
  untested.
- On a watch the support list only predicts to work, the app says under its status that no one
  has confirmed Georgian on that model yet and that Restore original font puts Garmin's fonts
  back, in Georgian and English, instead of the note for partly tested models.
- Georgian in Garmin's Korean bitmap fonts, for watches that list no font this app installed
  before: the Lily 2 Active gets it in `006-D8945-67/-68` (17 and 22 px, Compressed), the Venu
  (1st gen) in `006-D8945-22` (27 px, the font the Venu 2 drew Georgian from in the lab), the
  Instinct 3 AMOLED 50mm and 45mm in `006-D8945-07` (19 px). Three new packs, untested on any
  watch; these models stay "not yet confirmed", and restoring the original fonts on them is
  untested too. Each part is Garmin's file at the version the watch declares and Garmin's
  catalog serves; Instinct 3 parts declared at a version the catalog doesn't serve are left out.
- The app's check on the watch is no longer only on the restart screen right after installing.
  Connecting a watch that already has Georgian on a model and software no one has confirmed yet
  makes **Check on the watch** the main screen's main action (after the restart while Georgian
  still waits for one), and a confirmed watch gets a quiet **Didn't work? Check on the watch**.
  Before, clicking Done or closing the app during the restart left no way back to the check. On
  such a watch the restart and check screens say, with the model's name, that no one has
  confirmed Georgian on it yet and that a minute's check tells every owner of that model whether
  it works (or, confirmed on other software, that the answer tells whether it works on theirs).
  After sharing a check, the app says what happens next: when Georgian showed, that once posted
  the next version lists the watch as tested; otherwise, that the maintainer will look into it
  and reply on GitHub. The main screen no longer asks a watch with Georgian installed to share
  its font list "to help add this watch"; the check replaces it. The demo build adds a Lily 2
  Active with Georgian installed, an Instinct 3 AMOLED 50mm with it waiting, and a tested
  tactix 7 with it installed.

### Changed

- `issue_link`'s `report` is optional when `busy` is given (JSON input change ahead of 1.0).
- When the report is too long for the link, the issue form's Watch report box is left empty
  instead of holding a line to paste over, and the app says to paste into that empty box. GitHub
  then asks for the paste if it's missed; before, the instructions could be posted as the report
  (#35, #46). `issue_link`'s `fields` no longer lists `report` when it didn't fit.
- The Venu (1st gen, #12) and the Instinct 3 AMOLED 50mm and 45mm (#8, #33, #23) are no longer
  listed as not working: their reports declare Garmin's Korean bitmap fonts, so they are
  predicted to work through them (`bitmap_korean`) and move to the README's **Likely to work, not
  confirmed** table. The Instinct 3's old reason was wrong: it was grouped with the fenix 6 as
  having our font and not using it, but no report shows Georgian tried on it (#33 lists Garmin's
  own Swiss 721 Hebrew), and Swiss 721 Hebrew stays restore-only. The `cjk_bitmap_fonts` reason
  is gone with the Venu's entry. Install and restore on these models are untested.
- A watch gets Georgian in one family of fonts: bitmap Armenian if it lists any, else the
  TrueType fonts as before, else Korean. Watches already tested with Armenian fonts (fenix 7 Pro,
  Forerunner 255, epix Pro, Venu 2, Venu 2 Plus) keep exactly their install, Korean fonts
  untouched. Restore original font still puts back the restore-only Swiss 721 Hebrew fonts
  wherever a watch lists them, whatever family install chooses.

### Fixed

- Install and restore stop with `device_error`, writing nothing, when the watch lists Garmin's
  GetObjectIds but reading `ExtData`'s hidden listing fails, wholly or for one file; before, they
  went on as if the listing were complete, so an imported font they couldn't see could be
  replaced unchecked at the next restart. The development writes (`stage_file`, `mtp_create`,
  `mtp_delete`) refuse the same way. `extdata_list` reports such a partial listing as
  `hidden_listing: error` with its `note`.
- A bitmap part's imported `<part>.fnt` in the normal `ExtData` listing (as the watch in #38
  shows it) is now identified before install or restore writes a `.tmp` that would replace it at
  the next restart, and `inspect` reports it; before, only the hidden listing was checked.
- Install, restore and `inspect` recognize a part's files under Garmin's short name too
  (`D903502.tmp` for `006-D9035-02.tmp`, `.fnt` and `.bin` alike): a waiting file under either
  name is identified before anything is written, replaced when it is Garmin's or this
  program's, and refused otherwise, so a part never ends up with two waiting files. Two files
  for one part are named in the `conflict` error.
- Install and restore write each font under the file name the watch declares for it in
  `GarminDevice.xml`: the short `D894567.tmp` on a watch that declares short names, as the Lily 2
  Active does, so it imports `D894567.fnt`; the part number, `006-D9035-03.tmp`, on the Venu 2
  and every watch reported before. A waiting copy under the other name is moved to the declared
  one.

## [0.3.3] - 2026-11-23

### Added

- The Instinct 2 Solar Tactical (software 17.22, #38) in the support list as not working: it
  connects only as a USB drive, and its one optional font is an emoji font, with no letters to add
  Georgian to. Its report is a replay fixture.
- The Forerunner 255 on software 29.52 as tested (#13, #37): Georgian showed in pop-ups, the
  notification list and opened notifications with 0.2.2, whose eight bitmap Armenian builds are
  byte for byte today's. The app now shows it as confirmed and no longer asks it for a report on
  that version. Its 0.2.2 font list is a replay fixture.
- The fenix 5 (software 25.00, #30, #29) in the support list as not working: it connects only as
  a USB drive, and the app found no font on it that it can add Georgian to. Both reports are
  replay fixtures.
- 58 watch models predicted to work, not tested, in the support list with a `prediction`
  (`noto_truetype` or `bitmap_armenian`, high confidence only, with its basis and source) and in
  a new README table, **Likely to work, not confirmed**, grouped by model family. 36 come from
  Garmin's Connect IQ device reference listing Noto Sans Armenian (as on the Forerunner 165), 22
  from the fenix 7 firmware family (as the tested fenix 7 Pro). Where one device page names several
  part numbers, the entry carries that page's name.
- `inspect` and the watch report's `support` gain `prediction` (the predicted family, or null),
  and the report's `support` line adds `predicted=<family>`. A prediction changes neither the
  status nor `report_wanted`: a predicted watch is still judged by the fonts it lists.

### Changed

- The reason shown for watches that connect only as a USB drive (Forerunner 245, fenix 5) now
  says only what their reports show: the app found no font on the watch it can add Georgian to.
- The README's first watch table lists only reported models; predicted ones have their own.
- The Instinct 3 AMOLED 50mm is listed as not working on software 16.12 too (#33, a duplicate of
  #6), so the app no longer asks it for a report on that version. Its report is a replay fixture.
- The Forerunner 255's first font list (#35, software 29.52, a duplicate of #13) is a replay
  fixture.
- When a report is too long for the link, the app, the placeholder it puts in the form and the
  form itself now say to paste it into the Watch report field over that line, not into Notes
  (#35's went into Notes).

## [0.3.2] - 2026-11-22

### Added

- The README's **Help us test** section lists the watch groups we need a test watch for, with
  which ones we have, for donating or lending one in Tbilisi.

### Fixed

- After a restart, a watch with bitmap fonts that keeps its imports hidden, like the Venu 2, read
  as if Georgian weren't installed: the watch imports each staged `<part>.tmp` as a hidden
  read-only `<part>.fnt`, and `inspect` looked only at the `.tmp`. It now takes a bitmap part's
  file from its imported `.fnt` when no `.tmp` waits, so imported Georgian reads `installed`
  (the app offers Restore only), an imported earlier build `beta`, imported Garmin fonts
  `not_installed`, and an imported font this build doesn't recognise `other`; the report's
  `parts:` lines say the same. `inspect --json`'s `fonts` gain `imported`. Install and restore
  never delete the `.fnt`, and they refuse (`conflict`, nothing written) when it is a font they
  don't recognise, since the `.tmp` they stage would replace it at the next restart; the app
  says the watch uses a font for that part it doesn't recognise and asks for the font list.
  Found on our Venu 2 (19.05) with 0.3.1.
- The demo watch imports staged fonts the same way, so its Venu 2 shows Georgian installed on
  the next visit, and its Venu 3 then shows its unrecognised imported font.

## [0.3.1] - 2026-11-22

### Added

- The watch report lists `GARMIN/ExtData`'s hidden children: read-only objects the normal MTP
  listing leaves out, which Garmin's vendor `GetObjectIds` (0x9002) returns (the Venu 2's
  imported fonts; the `.bin` fonts the Forerunner 165 and fenix 6S Pro reports didn't show).
  Each gets its name, size and read-only status, and a file named for one of this build's parts
  what it holds (`georgian`, `georgian_beta`, `stock`, `other`). A failure is a note on the
  `hidden:` line, never an error. `inspect` and the report gain `extdata_hidden`.
- A support list in `fonts/registry.json` (`watches`): every watch model the project knows of,
  with its GitHub issues and, for one known not to work, why (`reasons`). It starts with the
  tested watches; the Forerunner 165 Music, Venu 3 and 3S (#5) and Forerunner 255 (#13) as not
  confirmed yet; and the fenix 6S Pro, 6 Pro Solar and 6X Pro and Instinct 3 AMOLED (#6),
  Forerunner 245 (#16), Forerunner 245 Music (#10) and Venu (#12) as not working. The README's
  watch table is generated from it (`python -m garmin_fonts.support`), and the builder tests
  fail when they differ.
- The app shows the connected watch's status from the list: tested, not confirmed yet
  (installs), or not working, with the reason and a **Follow updates** link to the issue. It
  asks for a report only when one adds something new: not for a tested watch, whose check
  screen says it's confirmed and offers **Didn't work? Share on GitHub** instead, nor for an
  unsupported one on a software version the list has. Sharing is still one click to the
  filled-in issue form.
- `inspect`, `inspect-drive` and the watch report gain `support`: `status` (`tested`,
  `expected`, `unsupported`, `unknown`), `listed`, `issue`, `reason` and `report_wanted`; the
  report text gets a `support` line, such as `support expected issue=#5`. `inspect` prints the
  status with the issue's link.
- A new error, `restore_first`: install refuses a TrueType file it can't read when the watch
  declares it below 1.00 (see Changed).
- Every watch report posted in the issues so far (26) is replayed in the tests against a fake
  watch built from it (`crates/core/tests/reports/`, made by `scripts/report_fixture.py`): install
  and restore must never write over a file that isn't Garmin's or ours, never delete a hidden
  object, and leave Garmin's originals wherever restore writes.
- The builder tests compare every Georgian build with Garmin's original: same table list,
  header and trailer, other tables byte for byte, every stock glyph, metric and cmap entry
  unchanged.

### Changed

- The fenix 6 and Instinct 3 family is unsupported in every build, betas included: on a fenix
  6S Pro our font reached the watch and the watch still drew question marks (#6). Swiss 721
  Hebrew (`006-D0952-07/-08`) is restore-only; restore still puts Garmin's fonts back.
- Betas install exactly what releases do: the registry's `install: "beta"` is gone, and a font
  family no watch has confirmed isn't installed by any build.
- Install and restore identify each file by its content, from the normal listing or, where the
  watch lists 0x9002, the hidden one: Garmin's original or one of this program's builds is
  replaced (a waiting `.tmp` with Garmin's original too, which install used to refuse), anything
  else is refused. A hidden read-only import is never deleted; a new file is written next to it.
  Only a file neither listing lets be read falls back to the declared version: install never
  writes over a file it can't identify and accepts only Garmin's version (below 1.00 it asks to
  restore first, `restore_first`), while restore may put Garmin's original back over a file
  without Garmin's version and accepts below 1.00 too, as watches declared this program's
  builds before 0.3.0 (0.00 on the fenix 6S Pro, #6; 0.01 on the Forerunner 165, #5). Install no
  longer accepts 0.01 by itself; restore now also accepts 0.00.
- `inspect`'s per-part `file` and `state` come from the hidden listing where the normal one shows
  nothing, as install and restore see it, so such parts no longer show as `not_visible`.
- Registry `tested` entries no longer carry `model`; a tested model's name comes from the
  support list, which must list it. The asset manifest gains `watches` (reasons written out);
  its `tested` entries lose `model`.

## [0.3.0] - 2026-11-22

### Added

- Forerunner 165 (29.52) as a tested watch, the first with TrueType fonts: Georgian in pop-ups,
  the notification list and opened notifications with an iPhone (#5, #19), with both its Armenian
  and Hebrew TrueType fonts installed. The beta's test mark showed the watch draws Georgian from the
  Armenian font `006-D9971-09`.
- **Check on the watch** after installing: a QR code for a new phone test page that sends a
  `Test მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ Test` notification, one-tap questions (what the watch showed, where,
  which phone), and **Share on GitHub**, which opens a new watch report issue form with every
  field filled in. The screen shows the exact report before the browser opens; a report too long
  for the link is copied for pasting instead. Sharing from the main screen and after a failed
  install or restore uses the same form ([Reporting](docs/reporting.md)).
- The phone test page (`site/`, published by GitHub Pages), in Georgian and English, with a copy
  fallback where the browser can't show notifications.
- Watches that connect as a USB drive instead of MTP, like the Forerunner 245 (#16): the app says
  Georgian can't be installed that way and offers to share their font list;
  `georgian-for-garmin inspect-drive` reads them.
- Beta builds draw a test mark into each font: one capital of ᲛᲗᲐᲕᲠᲣᲚᲘ gets a bar, in a
  different place per font file, so a notification shows which file the watch used. The check
  screen shows the installed marks as pictures to pick from. Stable builds' fonts are unchanged.
- The `issue_link` operation and `georgian-for-garmin issue-link`: the prefilled issue link from
  a report, the last install or restore, and the answers.
- The restart screen, README and Windows guide say to restart the watch once more if boxes still
  show (#4).
- Step-by-step Windows install guide in Georgian (`docs/install-windows.md`), with a screenshot of
  every screen from download to the watch's restart.
- Georgian for watches with TrueType fonts instead of bitmap Armenian ones: Noto Sans Armenian
  (`006-D9971-08/-09`) and Noto Sans Hebrew (`006-D1053-98/-99`) get Mkhedruli and Mtavruli,
  sized to each font's letters, confirmed on the Forerunner 165. This covers the Venu 3 and 3S
  and Forerunner 165 and 165 Music reported in issues #2 to #14. Only watches that list none of
  the bitmap Armenian fonts get them; a watch that does, like the fenix 7 Pro, installs exactly as
  before. Install replaces the watch's `GARMIN/ExtData/<part>.bin`, only when it is Garmin's
  original or a Georgian build of this program, reads it back, and writes every original back if
  any part fails; restore puts Garmin's original back.
- Swiss 721 Hebrew (`006-D0952-07/-08`, fenix 6 and Instinct 3 family) installs in beta builds
  only: Georgian in it showed question marks on a fenix 6S Pro (#6). Stable builds show those
  watches as unsupported, as before TrueType support, but still offer **Restore original font**
  so a beta tester can put Garmin's fonts back.
- Upgrading from a beta: install and restore replace the Georgian builds 0.3.0-beta.1 and beta.2
  installed (beta.2's test-marked `.tmp` and `.bin` files, beta.1's TrueType `.bin` files), which
  the registry lists by SHA-256 under `earlier_georgian`; `inspect` reports them as
  `georgian_beta`.
- `inspect`, the report and the app show what each part's file holds by one rule (`file`:
  `georgian`, `georgian_beta`, `stock`, `other`, `none`, `not_visible`, `unknown`) and sum it up as
  `state`. TrueType watches like the Forerunner 165 and fenix 6S Pro show no `.bin` files over
  MTP; their parts are `not_visible` and the app says it can't tell whether Georgian is installed,
  instead of offering to install as if nothing were there. Where it can see Georgian is installed,
  the main screen says so and offers only **Restore original font**.
- A "Question marks (?) instead of letters" answer, in the app and the issue form (#6).
- The report always says `marks beta` or `marks off`.
- `inspect` reports `hosts`: which kind of font (`bitmap` or `truetype`) gets Georgian on the
  watch, why, and the parts of the other kind it leaves alone.
- Venu 2 Plus (19.05) and fenix 7 Pro Solar / Pro Sapphire Solar (26.09) as tested watches, from
  user reports (#4, #11, #15). A model and software version now count as tested once someone
  confirms Georgian in a real notification; the entry lists the parts the app installed.

### Fixed

- TrueType Georgian builds keep the 16-byte trailer Garmin ends every `.bin` with (the version and
  part number). Without it watches listed the part as version 0.01, and restore stopped with
  `unsupported_device` on every Forerunner 165 and fenix 6S Pro that had installed a beta (#5, #6).
  Install and restore now treat a part listed at 0.01 as an earlier Georgian build and replace
  it. The TrueType builds' bytes change only by the added trailer.

### Changed

- JSON, ahead of 1.0: `inspect`'s `fonts[].staged` is now `fonts[].file` (values above), and
  `inspect` gains `state`, `restorable`, `fonts[].restore` and `hosts.restore_only_parts`; the
  report's `parts[].staged` is `parts[].file`, and it gains `state`.
- The watch report is now compact text for the issue form's report field, and lists every file
  `GarminDevice.xml` declares (path, file name, part, version), the `ExtData` listing, the host
  choice, each listed part's state, the OS and CPU architecture. JSON, ahead of 1.0: `report`
  gains `platform`, `connection`, `hosts`, `parts`, `test_marks`, `declared_files` and
  `extdata_listing`, and `fonts` entries gain `mark`. The Markdown issue template is replaced by
  the form.
- If the app can't find the watch, it says to choose MTP when the watch asks how to connect; some
  watches connect as a USB drive until you do. The README and the Windows guide say the same.
- JSON output, ahead of 1.0: packs and fonts carry a `kind`; TrueType parts have a null
  `size_px`; and `inspect` now gives a file's `sha256` only for files named for one of this
  build's parts with the size of one of its versions, `null` for all others, which it no longer
  reads. Watches with TrueType fonts keep files of several MB in `GARMIN/ExtData`, and the app
  inspects the watch every two seconds. Install and restore are declared destructive, since a
  TrueType install replaces a file.

## [0.2.2] - 2026-11-22

### Fixed

- On Windows, the app's window opens in front again when the installer's **Finish** starts it.
  Since 0.2.1 it opened behind the browser the installer was downloaded with, showing only its
  taskbar button. The window now starts hidden, is fitted to the screen, then shown and focused.

## [0.2.1] - 2026-11-22

### Fixed

- The app's window fits short screens. It was always 680 px tall, so at 1024x768 with 125%
  scaling, or on 1920x1080 laptops at 150%, its bottom, including the Done button, ran under the
  Windows taskbar. It now opens no taller than the screen's free area, centered in it, and the
  page scrolls when it doesn't fit; on taller screens it looks as before.

## [0.2.0] - 2026-11-21

### Added

- tactix 7 with software 27.18 as a tested watch: Georgian renders in its notifications with
  NotoSans Regular 19 px (`006-D9035-00`) and NotoSans Armenian Bold 13, 18, 20 and 25 px
  (`006-D9035-05`, `-06`, `-07`, `-09`), so `inspect` reports it as tested.

### Changed

- Release files no longer carry the version in their names (`Georgian-for-Garmin-macos.zip`, not
  `Georgian-for-Garmin-0.1.0-macos.zip`), so links to the latest release's files keep working.
  The README starts with download buttons for macOS, Windows and Linux that use them.

### Fixed

- The installer finds the hidden `GARMIN/ExtData` folder on watches where it lies past the 32
  handles it probes, such as the tactix 7 with software 27.18, whose `ExtData` is `0x01000021`.
  Before, `install` stopped with `extdata_not_found`. When the other searches fail and the watch
  lists Garmin's vendor operation `GetObjectIds` (`0x9002`), the installer asks it for
  `GARMIN`'s children, which include the ones listings leave out; `inspect --json` reports
  `found_by: "garmin_object_ids"`.

## [0.1.0] - 2026-11-20

### Added

- Guide for having an AI agent install Georgian on a Garmin watch without the app.
- Reproducible font build (`nix build .#fonts`): Georgian Mkhedruli and Mtavruli added to every
  Armenian font Garmin publishes, 20 files: NotoSans Regular (19–31 px), NotoSans Armenian Regular
  (11–37 px) and NotoSans Armenian Bold (13–25 px), each drawn at the weight and letter size of the
  font it joins; descenders are slightly shortened where a font's lines are too short for them.
  Ships with the original fonts for restoring and a manifest of hashes and the size each Georgian
  font is drawn at (`georgian_size_px`, `georgian_descender_percent`). The 21–31 px NotoSans
  Regular fonts are tested on a Venu 2 with firmware 19.05; the others aren't tested on a watch.
- Font registry (`fonts/registry.json`): the Garmin font packs that get Georgian, their download
  URLs and hashes, and the watch models each was tested on. Adding a pack is a registry entry;
  see `fonts/README.md`.
- `georgian-for-garmin` command-line installer. `install` and `restore-stock` stage the
  Georgian or original font in every Garmin Armenian font part the watch lists, across all packs,
  verify each by reading it back, and leave nothing behind if one fails; then unplug the watch and
  restart it. `--parts` limits it to given part numbers. Needs no libmtp or libusb. Runs on macOS,
  Windows and Linux.
- `inspect` shows the watch model, software version, each font pack it uses with its parts and
  whether each part was tested on this model and software, and what's staged in the hidden
  `GARMIN/ExtData` folder. `inspect --report` prints the watch's font list for a GitHub issue,
  without its serial number. The check text is `მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ`.
- `--json` output, `--input` JSON and `--help --json` tool definitions for scripts and agents.
- The installer recovers a watch left unresponsive by an earlier failed transfer, and
  `GEORGIAN_FOR_GARMIN_TRACE=1` prints an MTP trace to stderr.
- The installer core runs on Windows through Windows' built-in MTP driver (Windows Portable
  Devices), with no driver to install or replace. Verified from Windows 11 on Arm with a Venu 2.
- Linux: the command-line installer works with a udev rule
  (`packaging/linux/70-georgian-for-garmin.rules`); verified on Ubuntu 24.04 with a Venu 2. On
  Linux, a busy watch points to the GNOME or KDE file manager, and a new `permission_denied` error
  includes a one-line command that installs the rule.
- How it works, command-line and development guides in `docs/`.
- Desktop app for macOS, Windows and Linux in Georgian and English (Georgian by default; the
  choice is remembered).
  It finds the watch, shows its model, software version and whether it was tested, installs the
  Georgian font or restores the original with one click, and explains how to unplug and restart
  the watch. For untested watches, "Share my watch's font list" opens a prefilled GitHub issue.
- macOS release downloads: one app and command-line tool for both Apple silicon and Intel Macs,
  macOS 11 or later, with the fonts' manifest and SHA-256 checksums. Not notarized; ad-hoc
  signed only, as Apple silicon requires. `scripts/build-macos-release.sh` builds the same release
  locally.
- Windows downloads for x64 and Arm64 (Windows 10 and 11): a per-user installer for the app (no
  administrator rights; gets Microsoft WebView2 only if it's missing) and the command-line tool as a
  zip. Not signed: SmartScreen warns on first run.
- Linux downloads for x86_64 and aarch64: the app as a `.deb`, which installs the udev rule, and as
  an AppImage (both need glibc 2.34 or newer); the command-line tool as a static binary with the
  udev rule. `scripts/build-windows-release.ps1` and `scripts/build-linux-release.sh` build the
  same files locally.
- Releases carry one `SHA256SUMS` for every platform.

[Unreleased]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.3.4...HEAD
[0.3.4]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/lubaretsi/georgian-for-garmin/releases/tag/v0.1.0
