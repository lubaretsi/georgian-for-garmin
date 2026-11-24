# Georgian for Garmin

Adds Georgian letters to a Garmin watch's own optional font and installs it over USB: a font
builder, a Rust core, a CLI and a Tauri desktop app with a React UI.

## Layout

- `fonts/registry.json`: font packs, parts, pinned hashes, tested watches, the support list.
- `builder/`: the Python font toolkit (`garmin_fonts`) and its tests.
- `crates/core`: operations (one declaration each in `src/ops.rs`), sessions, MTP and WPD.
- `crates/cli`: the CLI, `rpc` and `mcp`. `crates/app`: the Tauri app; `src/demo.rs`, the demo.
- `crates/app/frontend`: the UI; its rules in `CONTRACT.md`, its words in `src/copy/{ka,en}.ts`.
- `crates/core/tests/`: the fake watch (`support/fake.rs`), report replays, UI fixtures.

## Commands

```sh
nix develop                     # toolchain; sets GEORGIAN_FOR_GARMIN_FONTS
nix flake check                 # registry, font build, builder tests
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd crates/app && cargo tauri dev --features demo     # the app on a simulated watch
pnpm -C crates/app/frontend install --frozen-lockfile
pnpm -C crates/app/frontend typecheck && pnpm -C crates/app/frontend test  # unit + stories
pnpm -C crates/app/frontend storybook                # http://localhost:6006
pnpm -C crates/app/frontend gen:types                # after an operation's types change
GFG_WRITE_UI_FIXTURES=1 cargo test -p georgian-for-garmin-core --test ui_fixtures
scripts/build-macos-release.sh  # outside nix develop; also build-linux-release.sh, .ps1
```

## Workflow

- Change `crates/app/frontend/CONTRACT.md` before the UI behaviour it describes.
- A new operation: its `ops.rs` declaration, a `session.rs` branch and a CLI subcommand.
- A new watch report fixture: `scripts/report_fixture.py` (see `crates/core/tests/reports/`).

## Conventions

- Commits follow Conventional Commits 1.0.0; branches are `feat/`, `fix/`, `hotfix/` or
  `chore/<slug>`.
- Public contents only: no credentials, raw captures, personal paths or helpers tied to one
  machine's hardware. Images are cropped to what they show.
- Every change users would notice adds a `CHANGELOG.md` entry in the same commit.
- Nix sees only Git-tracked files: `git add` new files before `nix build` or `nix flake check`.
- `src/core/types.gen.ts` and `frontend/fixtures/core/` are generated: regenerate, never edit.
- Tests that could reach a real watch go through the guard in `builder/tests/no_watch.py`.
- The `dev` feature writes any file to a watch: debug builds only, never in releases.
- Agents own this codebase: an improvement found while working gets made, in the same change
  when small and related, otherwise in its own PR.

## Code Review Rules

- A Georgian font a watch tested changes its bytes only with a new watch test. Safe path: keep
  the pinned SHA-256s in the builder tests and update them with the registry's `tested` entry;
  a new pack enters as `fonts/README.md` says (untested; installs once its family is confirmed).
- Nothing is written to a watch before every target is checked. Safe path: extend the core's
  pre-write checks and its fake-watch tests together.
- User-facing strings exist in both `ka` and `en`. Safe path: add to
  `crates/app/frontend/src/copy/en.ts` and `ka.ts` together; the compiler and the copy tests
  enforce it.
- Native rendering only: never suggest other apps, widgets, images or transliteration, anywhere.
  Safe path: an unsupported watch gets its reason and issue link, nothing else.

## More detail

- [Development](docs/development.md): setup, toolkit, Rust, demo, frontend, release
- [How it works](docs/how-it-works.md), [Command line](docs/command-line.md),
  [Reporting](docs/reporting.md)
- [Font registry](fonts/README.md), [UI contract](crates/app/frontend/CONTRACT.md)
