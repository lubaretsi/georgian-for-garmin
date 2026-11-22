# Report replay fixtures

Every watch report posted in this repository's issues and comments, as a fixture for
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
Forerunner 255's font list was never shared).

One fixture is composed by hand, because its report isn't in a report format, and names its
sources in `composed_from`: `issue-5-comment-7078836039.json` is the Forerunner 165's restore
failure in [#5](https://github.com/lubaretsi/georgian-for-garmin/issues/5#issuecomment-7078836039)
(app 0.3.0-beta.2, `unsupported_device`, the four TrueType parts declared 0.01, nothing written)
on the declared list of the same watch's report
[#19](https://github.com/lubaretsi/georgian-for-garmin/issues/19), with those four parts set to
0.01, an empty ExtData listing and no hidden listing (beta.2 didn't read it).
`the_0_01_state_restores_and_then_installs` checks it: install asks to restore first and writes
nothing, restore writes Garmin's originals, and after the restart install succeeds.

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

## What the fake watch does, and what showed it

| Behavior | Shown by |
|---|---|
| A written `<part>.tmp` waits in the normal listing until the watch restarts, which imports it and removes the `.tmp`. | The maintainer's Venu 2 ([How it works](../../../../docs/how-it-works.md#what-has-been-tested)) |
| An imported font becomes a hidden read-only object, `<part>.fnt` for a bitmap part, that only Garmin's `GetObjectIds` (0x9002) lists. | The Venu 2 ([Reporting](../../../../docs/reporting.md#hidden-files)); `006-D8168-00.fnt ro` on the fenix 6S Pro ([#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7080207810)) |
| Watches with TrueType fonts keep their `.bin` files out of the normal listing: the report declares them, `extdata:` is empty. | Forerunner 165 (#18, #19), fenix 6S Pro (#20) |
| A new `<part>.bin` written next to the hidden import is taken up at the restart and replaces it; the import itself is never deleted. | Forerunner 165: installed by 0.3.0-beta.2 with only the `.bin` write, Georgian showed (#19, #5) |
| After a restart the watch declares a `.bin`'s version from its 16-byte trailer: Garmin's version for Garmin's originals and this program's builds since 0.3.0; this program's trailer-less builds stay below 1.00. | 0.00 on the fenix 6S Pro ([#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7078829720)) and the fenix 6X Pro (2.10 right after installing, [#24](https://github.com/lubaretsi/georgian-for-garmin/issues/24); 0.00 after the restart, [#25](https://github.com/lubaretsi/georgian-for-garmin/issues/25)), 0.01 on the Forerunner 165 ([#5 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/5#issuecomment-7078836039), restore error) |
| Those hidden `.bin` files read back as this program's earlier builds. | `006-D0952-07/-08.bin ro georgian_beta` on the fenix 6S Pro ([#6 comment](https://github.com/lubaretsi/georgian-for-garmin/issues/6#issuecomment-7080207810)) |
| Some watches mount only as a USB drive and declare no ExtData fonts. | Forerunner 245 (#16) |

The fake can't know everything a report leaves out:

- A file's bytes come from what the report says it holds (`file`) or its SHA-256. Garmin's
  original and this build's Georgian are the bundled bytes; an earlier build's, which no report
  carries, are made-up bytes the test registers as an earlier Georgian build of that part, as
  the registry's `earlier_georgian` makes the real builds recognize 0.3.0-beta.1 and beta.2.
  Anything else counts as a file that is neither Garmin's nor ours.
- 0.1.x and 0.2.x reports didn't list the folder: the fake shows it empty and doesn't list
  0x9002.
- A report without a `hidden:` line (before 0.3.1-beta.1) gets a watch without 0x9002.
