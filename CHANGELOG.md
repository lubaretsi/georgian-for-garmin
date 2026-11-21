# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/lubaretsi/georgian-for-garmin/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lubaretsi/georgian-for-garmin/releases/tag/v0.1.0
