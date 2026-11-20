# Development

## Layout

| Path | What it is |
|---|---|
| `fonts/registry.json` | Garmin font packs that get Georgian: parts, download URLs, SHA-256s, tested watches ([how to add one](../fonts/README.md)) |
| `builder/` | Python font builder: decodes Garmin's font format, adds Georgian, writes the asset folder |
| `crates/core` | Rust library: the operations (`inspect`, `install`, `restore_stock`), the watch's font list, the hidden-folder search, and MTP over USB |
| `crates/cli` | The `georgian-for-garmin` command |
| `crates/app` | The desktop app (Tauri 2; plain HTML, CSS and JavaScript in `ui/`, strings in `ui/strings.json`) |
| `scripts/` | The macOS release build |
| `.github/workflows/` | `check.yml` on every push and pull request; `release.yml` on version tags |

## Set up

Install [Nix](https://nixos.org/download/) with flakes, then from the repository:

```sh
nix develop
```

The shell has Python with the builder's dependencies, Rust, `cargo-tauri`, and
`GEORGIAN_FOR_GARMIN_FONTS` pointing at the built fonts, which the Rust build embeds. Nix only
sees files tracked by Git: `git add` new files before building.

## Fonts

```sh
nix build .#fonts     # result/: georgian/, stock/, LICENSES/, manifest.json
nix flake check       # registry schema, builder tests, pinned SHA-256 of every Georgian font
```

The build downloads every stock font and Noto Sans Georgian by its pinned hash, so the output is
the same everywhere. A change that alters any Georgian font's bytes fails the tests; update the
pinned hashes only after testing the new fonts on a watch.

## Rust

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p georgian-for-garmin -- inspect
```

The core's tests run against an in-memory watch; no hardware is needed. To trace a real watch,
set `GEORGIAN_FOR_GARMIN_TRACE=1`.

## Desktop app

```sh
cd crates/app
cargo tauri dev                                            # against a real watch
cargo tauri build --debug --features demo --bundles app    # a simulated watch, no hardware
```

The demo build walks through a tested Venu 2, an untested model, a conflict and the share
button. Every string is in `ui/strings.json`, with `ka` and `en` tables that must have the same
keys; the tests check that.

## Release

Releases are built on macOS with Apple's toolchain, outside `nix develop`, so the binaries don't
depend on Nix. Nix only builds the fonts.

```sh
scripts/build-macos-release.sh     # needs Xcode command-line tools, rustup and Nix
```

It writes a universal app and CLI (Apple silicon and Intel, macOS 11 or later) to
`target/macos-release/dist/`, with the fonts' manifest and `SHA256SUMS`. It checks the binaries'
linked libraries, architectures, minimum macOS version, ad-hoc signatures and embedded fonts.

To publish, bump the workspace version, update `CHANGELOG.md`, and push a `v<version>` tag.
`release.yml` builds the same files and creates a draft release.
