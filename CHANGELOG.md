# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
- Georgian for watches with TrueType fonts instead of bitmap Armenian ones, untested on a watch
  so far: Noto Sans Armenian (`006-D9971-08/-09`), Noto Sans Hebrew (`006-D1053-98/-99`) and
  Swiss 721 Hebrew (`006-D0952-07/-08`) get Mkhedruli and Mtavruli, sized to each font's letters.
  This covers the Venu 3 and 3S, Forerunner 165 and 165 Music, fenix 6 Pro/6S Pro/6X Pro and
  Instinct 3 AMOLED reported in issues #2 to #14. Only watches that list none of the bitmap
  Armenian fonts get them; a watch that does, like the fenix 7 Pro, installs exactly as before.
  Install replaces the watch's `GARMIN/ExtData/<part>.bin`, only when it is Garmin's original or
  this build's Georgian font, reads it back, and writes every original back if any part fails;
  restore puts Garmin's original back.
- `inspect` reports `hosts`: which kind of font (`bitmap` or `truetype`) gets Georgian on the
  watch, why, and the parts of the other kind it leaves alone.
- Venu 2 Plus (19.05) and fenix 7 Pro Solar / Pro Sapphire Solar (26.09) as tested watches, from
  user reports (#4, #11, #15). A model and software version now count as tested once someone
  confirms Georgian in a real notification; the entry lists the parts the app installed.

### Changed

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

[Unreleased]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/lubaretsi/georgian-for-garmin/releases/tag/v0.1.0
