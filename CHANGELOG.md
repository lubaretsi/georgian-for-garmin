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
