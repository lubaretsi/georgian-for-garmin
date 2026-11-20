# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Guide for having an AI agent install Georgian on a Garmin watch without the app.
- Reproducible font build (`nix build .#fonts`): Garmin's 21, 24, 27 and 31 px Armenian fonts with
  Georgian Mkhedruli and Mtavruli added, the original fonts for restoring, and a manifest of hashes.
  The 27 px font is byte-identical to the one verified on a Venu 2 with firmware 19.05.
- Font registry (`fonts/registry.json`): the Garmin font packs that get Georgian, their download
  URLs and hashes, and the watch models each was tested on. Adding a pack is a registry entry;
  see `fonts/README.md`.
- `georgian-for-garmin` command-line installer for macOS. `install` and `restore-stock` stage the
  Georgian or original font in every size of the font pack the watch lists, verify each by reading
  it back, and leave nothing behind if one fails; then unplug the watch and restart it.
  `--sizes-px` limits the sizes. Needs no libmtp or libusb.
- `inspect` shows the watch model, software version, its font pack, whether that model and
  software were tested, and what's staged in the hidden `GARMIN/ExtData` folder. `inspect --report`
  prints the watch's font list for a GitHub issue, without its serial number.
- `--json` output, `--input` JSON and `--help --json` tool definitions for scripts and agents.
- The installer recovers a watch left unresponsive by an earlier failed transfer, and
  `GEORGIAN_FOR_GARMIN_TRACE=1` prints an MTP trace to stderr.
- The installer core runs on Windows through Windows' built-in MTP driver (Windows Portable
  Devices), with no driver to install or replace. Verified from Windows 11 on Arm with a Venu 2.
- Linux: the command-line installer works with a udev rule
  (`packaging/linux/70-georgian-for-garmin.rules`); verified on Ubuntu 24.04 with a Venu 2. On
  Linux, a busy watch points to the GNOME or KDE file manager and a permission error to the rule.
- How it works, command-line and development guides in `docs/`.
- Desktop app for macOS in Georgian and English (Georgian by default; the choice is remembered).
  It finds the watch, shows its model, software version and whether it was tested, installs the
  Georgian font or restores the original with one click, and explains how to unplug and restart
  the watch. For untested watches, "Share my watch's font list" opens a prefilled GitHub issue.
- macOS release downloads: one app and command-line tool for both Apple silicon and Intel Macs,
  macOS 11 or later, with the fonts' manifest and SHA-256 checksums. Not notarized; ad-hoc
  signed only, as Apple silicon requires. `scripts/build-macos-release.sh` builds the same release
  locally.
