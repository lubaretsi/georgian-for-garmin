# Report replay fixtures

Every watch report posted in this repository's issues and comments, and reports from the
maintainer's own watch, as a fixture for
`tests/replay.rs`. That test builds a fake watch from each fixture and runs inspect, install, a
restart, install again, a restart and restore against it with the real operations. It checks
that they never write over a file that is neither Garmin's original nor one of this program's
builds, never delete a hidden object, put Garmin's original back wherever restore writes, and
refuse with `unsupported_device` only a watch with nothing to write. Install may ask to restore
first (`restore_first`) only where it can't read a file the watch declares below 1.00.

## Adding a report

```sh
gh api repos/lubaretsi/georgian-for-garmin/issues/N --jq .body \
  | scripts/report_fixture.py --source https://github.com/lubaretsi/georgian-for-garmin/issues/N \
    --out crates/core/tests/reports/issue-N.json
```

For a comment, read `repos/…/issues/comments/ID` and name the file `issue-N-comment-ID.json`. The
script reads the compact report (0.3.x) or the Markdown of 0.1.x and 0.2.x, keeps only the
allowlisted fields (model, part number, software, app version, connection, the declared
`Garmin/ExtData` files, the ExtData and hidden listings) and refuses any value that looks like a
unit ID or serial number; `fixtures_hold_no_identifiers` checks the same. A prose report of a
watch that mounts as a drive needs `--connection usb_drive`. Issue #13 has no report (the
Forerunner 255's font list came later, in #37 and #35). The script finds the report anywhere in
the body, so one pasted into Notes (#35, #46) works too. A fixture may carry a `note` on what its report
says beyond the allowlisted fields: `issue-29.json`'s form answer was "Georgian letters",
although nothing was installed (`packs none`, connected as a USB drive); that is unexplained.

Two fixtures are composed by hand and name their sources in `composed_from`:

- `issue-5-comment-7078836039.json` is the Forerunner 165's restore failure in
  [#5](https://github.com/lubaretsi/georgian-for-garmin/issues/5#issuecomment-7078836039) (app
  0.3.0-beta.2, `unsupported_device`, the four TrueType parts declared 0.01, nothing written) on
  the declared list of the same watch's report
  [#19](https://github.com/lubaretsi/georgian-for-garmin/issues/19), with those four parts set to
  0.01, an empty ExtData listing and no hidden listing (beta.2 didn't read it).
  `the_0_01_state_restores_and_then_installs` checks it: install asks to restore first and
  writes nothing, restore writes Garmin's originals, and after the restart install succeeds.
- `maintainer-venu2-stock-imported.json` is the maintainer's Venu 2 after restore-stock and a
  restart: the report `maintainer-venu2-georgian-imported.json` (taken after install and a
  restart) with its four hidden `006-D9035-01..04.fnt` at the stock sizes the watch showed
  (13181, 16676, 20720 and 27831 bytes) and classified `stock`.

The two `maintainer-venu2-*` fixtures come from the maintainer's own watch, not an issue, and
say so in `source`. `the_venu_2_reads_its_imported_fonts` checks that imported Georgian reads
`installed` (0.3.1 read `not_installed`, every part `file none`) and imported Garmin fonts
`not_installed`.

`email-lily2active.json` is a Lily 2 Active's 0.3.2 report that its owner emailed the
maintainer, run through `scripts/report_fixture.py` like an issue's; its `source` and
`composed_from` say "maintainer email report, owner anonymized", and it holds nothing that names
the owner or the watch. It declares Chinese (`006-D8448-70/-71`), Japanese (`006-D8944-66/-67`)
and Korean (`006-D8945-67/-68`) bitmap fonts and an emoji font (`006-D8952-00`), all under
Garmin's short names (`D894567.tmp`), and lists its imports so (`D894567.fnt`).
`the_lily_2_active_round_trips_its_short_names` installs and restores a stand-in pack for its
Korean parts and checks that only the short names are written and imported;
`the_lily_2_active_never_writes_over_unknown_imports` checks that, with the imports' contents
unknown as reported, nothing is written.

The Instinct 3 AMOLED's reports (`issue-8`, `-23`, `-33`, `-55`, `-57`) are restore-only: 0.3.4
installed Georgian in its Korean `006-D8945-07` on two 45mm watches (#55, #57), and after the
restart notifications still didn't show it. `the_instinct_3_is_restore_only` checks each as
reported, with 0.3.4's `-07` waiting as a `.tmp`, and, where the report lists the imports, with
it imported (both composed: the 0.3.4 reports are from before installing): install refuses with
`unsupported_device` and writes nothing, and restore writes only pinned originals, putting
`-07`'s back wherever the watch holds our build.

| Fixture | Watch, software | App | Source |
|---|---|---|---|
| `issue-2.json` | Venu 3 (006-B4260-00), 17.05 | 0.1.0 | [#2](https://github.com/lubaretsi/georgian-for-garmin/issues/2) |
| `issue-3.json` | fenix 6X Pro (006-B3291-00), 28.02 | 0.2.0 | [#3](https://github.com/lubaretsi/georgian-for-garmin/issues/3) |
| `issue-4.json` | Venu 2 Plus (006-B3851-00), 19.05 | 0.2.0 | [#4](https://github.com/lubaretsi/georgian-for-garmin/issues/4) |
| `issue-5.json` | Forerunner 165 (006-B4432-00), 29.52 | 0.2.1 | [#5](https://github.com/lubaretsi/georgian-for-garmin/issues/5) |
| `issue-5-comment-7078836039.json` | Forerunner 165, 29.52, declaring 0.01; composed from the comment and #19's declared list | 0.3.0-beta.2 | [#5 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/5#issuecomment-7078836039), [#19](https://github.com/lubaretsi/georgian-for-garmin/issues/19) |
| `issue-6.json` | fenix 6S Pro (006-B3288-00), 28.02 | 0.2.1 | [#6](https://github.com/lubaretsi/georgian-for-garmin/issues/6) |
| `issue-6-comment-7078671921.json` | fenix 6S Pro, 28.02, before installing | 0.3.0-beta.2 | [#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7078671921) |
| `issue-6-comment-7078767795.json` | fenix 6S Pro, 28.02, after installing | 0.3.0-beta.2 | [#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7078767795) |
| `issue-6-comment-7078829720.json` | fenix 6S Pro, 28.02, declaring 0.00 | 0.3.0-beta.2 | [#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7078829720) |
| `issue-6-comment-7080207810.json` | fenix 6S Pro, 28.02, with the hidden listing | 0.3.1-beta.1 | [#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7080207810) |
| `issue-7.json` | Forerunner 165 Music (006-B4433-00), 29.52 | 0.2.1 | [#7](https://github.com/lubaretsi/georgian-for-garmin/issues/7) |
| `issue-8.json` | Instinct 3 AMOLED 50mm (006-B4587-00), 15.19 | 0.2.2 | [#8](https://github.com/lubaretsi/georgian-for-garmin/issues/8) |
| `issue-9.json` | Venu 3S (006-B4261-00), 17.05 | 0.2.2 | [#9](https://github.com/lubaretsi/georgian-for-garmin/issues/9) |
| `issue-10.json` | Forerunner 245 Music (006-B3077-00), 13.70 | 0.2.2 | [#10](https://github.com/lubaretsi/georgian-for-garmin/issues/10) |
| `issue-11.json` | fenix 7 Pro Solar (006-B4375-00), 26.09 | 0.2.2 | [#11](https://github.com/lubaretsi/georgian-for-garmin/issues/11) |
| `issue-12.json` | Venu (006-B3226-00), 7.80 | 0.2.2 | [#12](https://github.com/lubaretsi/georgian-for-garmin/issues/12) |
| `issue-14.json` | fenix 6 Pro Solar (006-B3290-00), 28.02 | 0.2.2 | [#14](https://github.com/lubaretsi/georgian-for-garmin/issues/14) |
| `issue-15.json` | fenix 7 Pro Sapphire Solar (006-B4375-00), 26.09 | 0.2.2 | [#15](https://github.com/lubaretsi/georgian-for-garmin/issues/15) |
| `issue-16.json` | Forerunner 245 (006-B3076-00), 13.70, a USB drive | none | [#16](https://github.com/lubaretsi/georgian-for-garmin/issues/16) |
| `issue-18.json` | Forerunner 165 (006-B4432-00), 29.52 | 0.3.0-beta.2 | [#18](https://github.com/lubaretsi/georgian-for-garmin/issues/18) |
| `issue-19.json` | Forerunner 165, 29.52, after installing | 0.3.0-beta.2 | [#19](https://github.com/lubaretsi/georgian-for-garmin/issues/19) |
| `issue-20.json` | fenix 6S Pro (006-B3288-00), 28.02 | 0.3.0-beta.2 | [#20](https://github.com/lubaretsi/georgian-for-garmin/issues/20) |
| `issue-22.json` | Forerunner 165 (006-B4432-00), 29.52 | 0.2.2 | [#22](https://github.com/lubaretsi/georgian-for-garmin/issues/22) |
| `issue-23.json` | Instinct 3 AMOLED 45mm (006-B4586-00), 16.12 | 0.2.2 | [#23](https://github.com/lubaretsi/georgian-for-garmin/issues/23) |
| `issue-24.json` | fenix 6X Pro (006-B3291-00), 28.02, right after installing (declared 2.10) | 0.3.0-beta.2 | [#24](https://github.com/lubaretsi/georgian-for-garmin/issues/24) |
| `issue-25.json` | fenix 6X Pro, 28.02, after the restart (declared 0.00) | 0.3.0-beta.2 | [#25](https://github.com/lubaretsi/georgian-for-garmin/issues/25) |
| `issue-29.json` | fenix 5 (006-B2697-00), 25.00, a USB drive; answered "Georgian letters" with nothing installed (see above) | 0.3.2 | [#29](https://github.com/lubaretsi/georgian-for-garmin/issues/29), closed as a duplicate of #30 |
| `issue-30.json` | fenix 5 (006-B2697-00), 25.00, a USB drive | 0.3.2 | [#30](https://github.com/lubaretsi/georgian-for-garmin/issues/30) |
| `issue-33.json` | Instinct 3 AMOLED 50mm (006-B4587-00), 16.12, with the hidden listing | 0.3.2 | [#33](https://github.com/lubaretsi/georgian-for-garmin/issues/33), closed as a duplicate of #6 |
| `issue-35.json` | Forerunner 255 (006-B3992-00), 29.52, before installing: 8 bitmap Armenian parts, all Garmin's, with the hidden listing | 0.3.2 | [#35](https://github.com/lubaretsi/georgian-for-garmin/issues/35) (report in Notes), closed as a duplicate of #13 |
| `issue-37.json` | Forerunner 255 (006-B3992-00), 29.52, the same 8 bitmap Armenian parts (`006-D9035-00` declared twice, as the watch listed it) | 0.2.2 | [#37](https://github.com/lubaretsi/georgian-for-garmin/issues/37), the font list for #13's confirmation |
| `issue-38.json` | Instinct 2 Solar Tactical (006-B3888-00), 17.22, a USB drive; one emoji font, `006-D8952-00` | 0.3.2 | [#38](https://github.com/lubaretsi/georgian-for-garmin/issues/38) |
| `issue-46.json` | epix Pro (Gen 2) 51mm (006-B4314-00, "EPIX PRO - 51mm"), 27.18, before installing: bitmap Armenian `006-D9035-09`, `-12`, `-13`, `-14`, with the hidden listing | 0.3.3 | [#46](https://github.com/lubaretsi/georgian-for-garmin/issues/46) (report in Notes) |
| `issue-55.json` | Instinct 3 AMOLED 45mm (006-B4586-00, "Instinct 3 - 45mm, AMOLED"), 16.12, from before installing, with the hidden listing; install then `staged_verified` `006-D8945-07`, and after the restart the answer was question marks | 0.3.4 | [#55](https://github.com/lubaretsi/georgian-for-garmin/issues/55), closed as a duplicate of #57 |
| `issue-57.json` | Instinct 3 AMOLED 45mm (006-B4586-00), 16.12, as #55, the answer boxes ("squares with question marks" in its comment) | 0.3.4 | [#57](https://github.com/lubaretsi/georgian-for-garmin/issues/57) |
| `issue-69.json` | Venu 3 (006-B4260-00), 17.51, its fonts as read before installing: TrueType `006-D9971-09` and `006-D1053-98` `stock`, an empty normal listing, with the hidden listing; install then `staged_verified` both, and the answer was Georgian letters in pop-ups and opened notifications (Android) | 0.4.1 | [#69](https://github.com/lubaretsi/georgian-for-garmin/issues/69) |
| `maintainer-venu2-georgian-imported.json` | Venu 2 (006-B3703-00), 19.05, after install and a restart: Georgian imported | 0.3.1 | the maintainer's watch, 2026-11-22 |
| `maintainer-venu2-stock-imported.json` | Venu 2, 19.05, after restore-stock and a restart: Garmin's fonts imported; composed | 0.3.1 | the maintainer's watch, 2026-11-22 |
| `email-lily2active.json` | Lily 2 Active (006-B4477-00), 5.10, before installing: CJK bitmap fonts and an emoji font under short names, with the hidden listing | 0.3.2 | maintainer email report, owner anonymized |

## What the fake watch does, and what showed it

| Behavior | Shown by |
|---|---|
| A written `<part>.tmp` waits in the normal listing until the watch restarts, which imports it and removes the `.tmp`. | The maintainer's Venu 2 ([How it works](../../../../docs/how-it-works.md#what-has-been-tested)) |
| An imported font becomes a hidden read-only object, `<part>.fnt` for a bitmap part, that only Garmin's `GetObjectIds` (0x9002) lists. | The Venu 2 ([Reporting](../../../../docs/reporting.md#hidden-files)); `006-D8168-00.fnt ro` on the fenix 6S Pro ([#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7080207810)); on the Instinct 3 AMOLED 50mm, 16.12, every declared ExtData font, the bitmap `.fnt` and TrueType `.bin` files (`006-D0952-07/-08.bin ro stock`), while its normal listing is empty ([#33](https://github.com/lubaretsi/georgian-for-garmin/issues/33)) |
| The import keeps the `.tmp`'s bytes unchanged and replaces the earlier `<part>.fnt`: Georgian, then Garmin's original, then Georgian again, each time with the staged file's size and SHA-256. | The maintainer's Venu 2, 19.05, with the 0.3.1 CLI (2026-11-22): hidden `006-D9035-01..04.fnt` held our Georgian build; restore-stock staged Garmin's `.tmp` (13181, 16676, 20720, 27831 bytes) and after the restart the `.fnt` had those sizes and hashes and the watch showed boxes; install staged Georgian and after the next restart the `.fnt` were Georgian again (23101, 28341, 35638, 48004 bytes) and the watch showed Georgian |
| A watch that declares its fonts under Garmin's short names keeps their imports under the same short names: a staged `D894567.tmp` becomes `D894567.fnt`. | The Lily 2 Active, 5.10 (`email-lily2active.json`): declares `D894567.tmp` and the rest, lists `D894567.fnt ro` and the rest. That it imports a `.tmp` under the name it declares is inferred from that; whether it would import a long-named one is unknown, which is why install writes the declared name |
| Watches with TrueType fonts keep their `.bin` files out of the normal listing: the report declares them, `extdata:` is empty. | Forerunner 165 (#18, #19), fenix 6S Pro (#20), Instinct 3 AMOLED 50mm (#33, its declared `.tmp` bitmap fonts too) |
| A new `<part>.bin` written next to the hidden import is taken up at the restart and replaces it; the import itself is never deleted. | Forerunner 165: installed by 0.3.0-beta.2 with only the `.bin` write, Georgian showed (#19, #5) |
| After a restart the watch declares a `.bin`'s version from its 16-byte trailer: Garmin's version for Garmin's originals and this program's builds since 0.3.0; this program's trailer-less builds stay below 1.00. | 0.00 on the fenix 6S Pro ([#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7078829720)) and the fenix 6X Pro (2.10 right after installing, [#24](https://github.com/lubaretsi/georgian-for-garmin/issues/24); 0.00 after the restart, [#25](https://github.com/lubaretsi/georgian-for-garmin/issues/25)), 0.01 on the Forerunner 165 ([#5 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/5#issuecomment-7078836039), restore error) |
| Those hidden `.bin` files read back as this program's earlier builds. | `006-D0952-07/-08.bin ro georgian_beta` on the fenix 6S Pro ([#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7080207810)) |
| Some watches mount only as a USB drive and declare no ExtData fonts, or only an emoji font. | Forerunner 245 (#16), fenix 5 (#30, #29: `extdata not found`), Instinct 2 Solar Tactical (#38: `006-D8952-00.fnt`, Garmin's Openmoji-Black 22px emoji font, 97,945 bytes) |

The fake can't know everything a report leaves out:

- A file's bytes come from what the report says it holds (`file`) or its SHA-256. Garmin's
  original and this build's Georgian are the bundled bytes; an earlier build's, which no report
  carries, are made-up bytes the test registers as an earlier Georgian build of that part, as
  the registry's `earlier_georgian` makes the real builds recognize 0.3.0-beta.1 and beta.2.
  Anything else counts as a file that is neither Garmin's nor ours.
- 0.1.x and 0.2.x reports didn't list the folder: the fake shows it empty and doesn't list
  0x9002.
- A report without a `hidden:` line (before 0.3.1-beta.1) gets a watch without 0x9002.
