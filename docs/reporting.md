# Reporting a watch

Every watch report is a public GitHub issue, filed through the
[watch report form](../.github/ISSUE_TEMPLATE/watch-report.yml). A GitHub account is all it
takes; there is no server of ours in between. The app fills in the form, so the owner only checks
it and submits it.

## In the app

1. After **Install Georgian** the restart screen has **Check on the watch**.
2. The check screen shows a QR code for the [phone test page](#the-phone-test-page) (drawn in the
   app, without the network) and the test text to send yourself instead.
3. One tap per question: what the watch showed (Georgian letters, boxes, question marks, some of each, the
   notification didn't arrive, Georgian didn't install), where Georgian showed (pop-up,
   notification list, opened notification), the phone (Android or iPhone), and in beta builds
   which [test mark](#beta-test-marks) the watch showed, from pictures of the marks the app
   installed.
4. **What's shared** shows the exact report text before anything opens. **Share on GitHub** opens
   the form with every field filled in. On a watch already confirmed on its model and software
   (`tested`), the screen says so and the button becomes the secondary **Didn't work? Share on
   GitHub**: only a failure is news there.

The same form is used by:

- **Share my watch's font list** on the main screen, only when a report from the watch adds
  information (report only, no answers; see below);
- **Report this on GitHub** on the error screen after a failed install or restore (the error code
  is in the report);
- the screen for watches that connect as a [USB drive](#watches-that-connect-as-a-usb-drive),
  under the same rule.

### When the app asks

Each watch has a status from the support list in `fonts/registry.json`
([How it works](how-it-works.md#the-watchs-font-folder)), and the app asks for a report only when
one would add something new:

| Status | Asks for a report |
|---|---|
| `tested` | No; after installing, only if Georgian didn't work |
| `expected` | Yes: install, then **Check on the watch** |
| `unsupported` | Only on a software version the list doesn't have; otherwise the main screen shows the reason and a **Follow updates** link to the issue |
| `unknown` | Yes: its font list |

Sharing is always one click to a new issue with the form filled in. The maintainer closes
duplicates into a watch family's main issue (the first issue a model lists), which is the one the
app links.

From the command line: `georgian-for-garmin inspect --report` prints the same report text, and
`issue-link` makes the link ([Command line](command-line.md#commands)).

## The form

Field ids are the new-issue URL's query parameters: `result`, `views`, `marker`, `phone`,
`connect` (Garmin Connect version), `report` and `notes`, plus `title`. The form adds the
`watch-report` label. GitHub documents prefilling only for text fields, so the answer fields
(dropdowns and checkboxes) are passed by their option labels but none is required, and the report
field repeats every answer on its `answers` line: if GitHub ignores a dropdown parameter, nothing
is lost and Submit still works.

## Link length

GitHub refuses URLs somewhat above 8 KB (414). The report is kept short: one item per line,
space-separated columns, no Markdown table syntax (the form shows it as a code block). The Venu 2
declares 70 files; its report is about 3.6 KB and the whole link, answers included, about 4.3 KB.
If the link would still pass 8,000 characters (a watch that declares
hundreds of files, say), the report is left out of it: the app copies it to the clipboard, every
other field is still filled in, and the report field says to paste it there. The check screen says
so before opening the browser.

## What's in a report

Only these fields; the unit ID, serial number and everything else in `GarminDevice.xml` stay out
(the core tests check that a report's keys are exactly these and that identifiers in the XML never
appear):

| Field | Source |
|---|---|
| `app_version`, `platform` (`os`, `os_version`, `arch`) | This program and the computer it runs on |
| `connection` | `mtp` or `usb_drive` |
| `model`, `model_part`, `software_version` | `GarminDevice.xml`: `Model/Description`, `Model/PartNumber`, `Model/SoftwareVersion` |
| `declared_files` (`path`, `file_name`, `part_number`, `version`) | Every `MassStorageMode/UpdateFile` |
| `extdata_files` | The same, for `Garmin/ExtData` only |
| `extdata_listing` (`name`, `bytes`, `sha256`) | The `GARMIN/ExtData` folder; `sha256` only for files `inspect` already reads (named for a bundled part, with one of its sizes) |
| `extdata_hidden` (`status`, `files`: `name`, `bytes`, `read_only`, `file`; `note`) | `GARMIN/ExtData`'s children the normal listing leaves out, from Garmin's `GetObjectIds`; `file` only for this build's parts |
| `hosts` (`kind`, `reason`), `packs`, `tested`, `state` | This build's choice of fonts for the watch, and Georgian on it |
| `support` (`status`, `listed`, `issue`, `reason`, `report_wanted`) | The watch's status in the support list; the `support` line, such as `support expected issue=#5`, adds `unlisted` for a model the list doesn't have and `new_software` for an unsupported model on a new software version |
| `parts` (`part`, `target`, `file`, `tested`, `mark`) | Each bundled part the watch lists |
| `test_marks` | Whether this is a beta build with test marks (`marks beta` or `marks off`) |

The link adds the last install or restore on this watch (`last`: its status and parts, or its
error code) and the answers.

An example (`inspect --report`, a Venu 2 before installing):

```text
app 0.3.0 "Mac OS" 15.6.1 aarch64
connection mtp
watch "Venu 2" 006-B3703-00 19.05
support tested
hosts bitmap: This watch lists bitmap fonts this build adds Georgian to, so Georgian goes into those.
packs armenian-noto-sans-regular (tested)
state not_installed
marks off
parts: part file target tested mark
006-D9035-01 none yes tested -
…
declared: path file part version
Garmin/ExtData 006-D9035-01.tmp 006-D9035-01 2.10
…
extdata: name bytes sha256
…
hidden: 006-D9035-03.fnt 35638 ro georgian; 006-D8448-25.fnt 21302 ro
```

`state` and each part's `file` follow one rule, the same in the app and the CLI. `file` is what
the part's file holds: `georgian`, `georgian_beta` (an earlier release's build), `stock`, `other`,
`none`, `not_visible` (a TrueType part the watch declares but doesn't show over MTP, as on the
Forerunner 165 and fenix 6S Pro) or `unknown`. `state` sums up the parts install writes:
`installed`, `waiting` (bitmap, until the restart), `partly`, `beta`, `not_installed`,
`not_visible`, `stock_waiting`, `other`, `unknown` or `unsupported`. A bitmap part's waiting
`.tmp` decides what it holds; without one, the `<part>.fnt` the watch imported at its last
restart does, so imported Georgian reads `installed`, imported Garmin fonts `not_installed`, and
an imported font this build doesn't recognise `other`: install and restore refuse it, since a
new `.tmp` would replace it at the next restart, and the app asks for the font list
(`inspect --json` marks such a part `imported`). With a last install or restore, the state and
files are from just before it, and the lines say so.

### Hidden files

Some watches leave read-only objects out of the normal MTP listing: on the Venu 2, Garmin's vendor
`GetObjectIds` (`0x9002`) on a folder returns exactly those, such as the imported `.fnt` fonts in
`ExtData`, and the two listings together cover the folder. The Forerunner 165 and fenix 6S Pro
reports show an empty `extdata:` while declaring 11 `.bin` fonts: the same hiding, as the fenix 6S
Pro's hidden listing later showed (#6). When the
watch lists `0x9002`, `inspect` lists `ExtData`'s hidden children too, on the report's `hidden:`
line: each file's name, size and `ro` (read-only), `rw`, or `?` when the protection couldn't be
read (on Windows it comes from the watch's own ObjectInfo, since WPD has no property for it), and for a file named for one of this
build's parts on the watch (a bitmap part also as its imported `<part>.fnt`) what it holds, by
the same rule as `file`. Only those few files are read. `hidden: unsupported` means the watch
doesn't list the operation, `hidden: error …` that listing or a read failed (the rest of the
report is unaffected), and `hidden: not read` that `ExtData` wasn't found or the watch is a USB
drive.

`inspect` takes a part's `file`, and so `state`, from it where the normal listing shows nothing:
a TrueType part's hidden `.bin`, or the `<part>.fnt` the watch imported a bitmap part's `.tmp`
as. Install and restore identify those imports the same way and refuse one that is neither
Garmin's nor ours. A hidden object is never deleted: install and restore write a new file next
to it (a bitmap part's new `.tmp`), which the watch takes up when it restarts
([How it works](how-it-works.md#truetype-fonts)).

## The phone test page

<https://lubaretsi.github.io/georgian-for-garmin/> ([site/](../site)) has one button, **Send test
notification**, which shows a notification with `Test მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ Test`, and a **Copy
text** fallback with a line on sending it to yourself. It is in Georgian with an English switch,
loads nothing from other sites and keeps only the language choice.

- On Android the watch shows it only if Garmin Connect forwards the browser's notifications
  (the browser is on Garmin Connect's list of apps that notify the watch).
- iPhone Safari shows web notifications only for pages added to the Home Screen (iOS 16.4 and
  later); otherwise the page puts the copy fallback first.

**One-time setting for the repository owner:** Settings → Pages → Build and deployment → Source:
**GitHub Actions**. The `Pages` workflow (`.github/workflows/pages.yml`) then publishes `site/` on
every push to `main` that changes it.

## Beta test marks

In pre-release builds (the version has a pre-release identifier, as in `0.3.0-beta.2`; the flake's
`fonts-marked` builds them explicitly), each part's Georgian font draws one capital of
`ᲛᲗᲐᲕᲠᲣᲚᲘ` with a bar, in a different place for every part of a kind. The rest of the test
text is normal Mkhedruli and Mtavruli, so one notification answers two questions: whether Georgian
works, and which font file the watch used for it. Stable builds draw no marks and are byte for
byte the fonts pinned in the builder tests.

Parts are numbered in registry order within each kind (bitmap A–T, TrueType A–F; a watch gets
only one kind): letter _n_ of `ᲛᲗᲐᲕᲠᲣᲚᲘ` (_n_ = index mod 8) carries the bar, below the letter
for the first eight, above it for the next eight, and both for the rest. The build manifest
records each part's mark (`mark` in `manifest.json`), the report lists it per part, and the app
shows the installed parts' marks as pictures for the marker question.

![Every part's test mark, drawn from its own Georgian build](images/test-marks.png)

## Watches that connect as a USB drive

Some watches mount as a drive instead of MTP (the Forerunner 245 in issue #16). The app finds a
mounted volume with `GARMIN/GarminDevice.xml` when no MTP watch is connected, says that Georgian
can't be installed this way, and offers the same report, read-only (the `ExtData` listing without
hashes). From the command line: `georgian-for-garmin inspect-drive --report`.
