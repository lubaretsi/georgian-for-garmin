# Georgian for Garmin

Adds Georgian letters to a Garmin watch's own optional font and installs it over USB: a font
builder, a Rust core, a CLI and a Tauri desktop app.

## Layout

- `fonts/registry.json`: font packs, parts, pinned hashes, tested watches.
- `builder/`: Python font builder. `crates/core`: operations and MTP. `crates/cli`, `crates/app`.
- Details: [docs/development.md](docs/development.md).

## Commands

```sh
nix develop                     # toolchain; sets GEORGIAN_FOR_GARMIN_FONTS
nix flake check                 # registry, font build, builder tests
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
scripts/build-macos-release.sh  # outside nix develop
```

## Conventions

- Commits follow Conventional Commits 1.0.0; branches are `feat/`, `fix/`, `hotfix/` or
  `chore/<slug>`.
- Public contents only: no credentials, raw captures, personal paths or helpers tied to one
  machine's hardware. Images are cropped to what they show.
- Every change users would notice adds a `CHANGELOG.md` entry in the same commit.
- Nix sees only Git-tracked files: `git add` new files before `nix build` or `nix flake check`.
- Agents own this codebase: an improvement found while working gets made, in the same change
  when small and related, otherwise in its own PR.

## Code Review Rules

- A Georgian font's bytes change only with a watch test. Safe path: keep the pinned SHA-256s in
  the builder tests and update them in the same change as the registry's `tested` entry.
- Nothing is written to a watch before every target is checked. Safe path: extend the core's
  pre-write checks and its fake-watch tests together.
- User-facing strings exist in both `ka` and `en`. Safe path: add to `crates/app/ui/strings.json`
  in both tables; the app tests enforce it.
- Native rendering only: never suggest other apps, widgets, images or transliteration, anywhere.
  Safe path: an unsupported watch gets its reason and issue link, nothing else.

## More detail

- [How it works](docs/how-it-works.md)
- [Command line](docs/command-line.md)
- [Font registry](fonts/README.md)
