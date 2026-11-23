# Development

## Layout

| Path | What it is |
|---|---|
| `fonts/registry.json` | Garmin font packs that get Georgian, their host families, other pinned stock fonts: parts, download URLs, SHA-256s, tested watches, and the support list of every known watch model ([how to add one](../fonts/README.md)) |
| `fonts/fallback-models.json` | Which font a watch view draws a character from, as far as recorded observations show, with citations ([Fallback models](../fonts/README.md#fallback-models)) |
| `builder/` | Python font toolkit (`garmin_fonts`): Garmin's font formats, the stock font catalog, adding Georgian, rendering as a watch would, the watch over `rpc`; writes the asset folder |
| `crates/core` | Rust library: the operations (`inspect`, `install`, `restore_stock`, `extdata_list`, …), sessions that keep the watch open (`session.rs`), the JSON-RPC loop (`rpc.rs`), the watch's font list, the hidden-folder search, and MTP over USB |
| `crates/cli` | The `georgian-for-garmin` command, with its `rpc` and `mcp` (`mcp.rs`, the official Rust MCP SDK `rmcp`) sessions |
| `crates/app` | The desktop app (Tauri 2; plain HTML, CSS and JavaScript in `ui/`, strings in `ui/strings.json`) |
| `scripts/` | The release builds (macOS, Windows, Linux) |
| `.github/workflows/` | `check.yml` on every push and pull request; `release.yml` on version tags and manual runs |

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
pinned hashes only after testing the new fonts on a watch. The builder tests also round-trip
every pinned stock font through the codec (plain and "Compressed") and render the app's test
text over every watch report fixture, with Garmin's fonts and with ours; the flake check's
output, `render/`, holds those PNGs and `report.json` (each character `confirmed`,
`coverage_only` or `unknown`), which CI uploads as the `render-report` artifact. The report
fails only on what is knowable: a confirmed step that doesn't hold, or one of our installed
fonts lacking a Georgian letter.

### Font toolkit

Experiments on fonts and watches run as steps through `garmin_fonts`, from `builder/` in
`nix develop`, in Python (omp's eval, see below, or a REPL) or on the command line:

```python
from garmin_fonts.catalog import fetch_stock
from garmin_fonts.build import add_georgian
from garmin_fonts.render import render, render_report
from garmin_fonts.watch import Watch

thai = add_georgian("006-D8947-22", markers={"ჰ": "გ", "ก": "ข"})   # fetched, sized like Thai
rendering = render_report("../crates/core/tests/reports/maintainer-venu2-stock-imported.json",
                          replace=[thai])   # builds=[…] only takes what install would write
rendering.counts(), rendering.trace[5], rendering.save("venu2.png")

with Watch("../target/debug/georgian-for-garmin") as watch:   # a `--features dev` build
    watch.summary(); watch.hash("006-D8947-22.fnt")
    staged = watch.stage(thai)                # GARMIN/ExtData/006-D8947-22.tmp, read back
    watch.remove_staged(staged["id"])
```

Georgian's bytes depend on the libraries that draw it: Pillow from PyPI lacks raqm, measures a
glyph's advance with its basic layout engine instead, and with the same FreeType (2.14.3) gives
22 of the 66 glyphs of the Korean 27 px build another width. Each build records its
`rasterizer` and whether it is the pinned one (`fonts/registry.json`,
`georgian_font.rasterizer`, the `nix develop` environment); elsewhere `add_georgian` warns and
`Watch.stage` refuses the build unless `allow_unpinned_env=True`. `builder/pyproject.toml`
lists the dependencies for use outside nix.

`codec.round_trips` re-compresses every glyph of a "Compressed" font in pure Python: 10 to 30
seconds for the large ones; plain fonts take milliseconds.

### Using the toolkit from omp's eval

omp's eval kernel runs `<cwd>/.venv/bin/python` when the session's directory has a `.venv`. From
the checkout (or a worktree, each gets its own):

```sh
nix run .#omp-venv
```

It writes `.venv/` (ignored by Git; it refuses to replace a `.venv` it didn't write): a
`bin/python` that runs the builder's pinned Python, the one the fonts derivation uses, with
`builder/` on the path and the pinned font inputs (`GFG_STOCK_DIR`, `GFG_CATALOG_DIR`, Noto Sans
Georgian) set, plus a garbage-collector root for them. Run it again after the flake's inputs
change.

omp picks the interpreter when a session first runs Python in a directory and keeps it: run
`nix run .#omp-venv` before the first eval cell, or start a new session after. For a session
started elsewhere, either launch omp with `VIRTUAL_ENV=<checkout>/.venv` (omp checks it before
`<cwd>/.venv`), or point omp's `python.interpreter` setting at `<checkout>/.venv/bin/python`.

```python
from garmin_fonts.build import add_georgian
from garmin_fonts.render import render_report, REPORTS
from garmin_fonts.watch import Watch
korean = add_georgian("006-D8945-22")               # pinned stock, sized like Korean
print(korean)                                       # rasterizer_pinned=True in this .venv
rendering = render_report(REPORTS / "maintainer-venu2-stock-imported.json", replace=[korean])
print(rendering, rendering.image)                   # the counts, then the picture inline
with Watch("target/debug/georgian-for-garmin") as watch:   # a `--features dev` build
    print(watch.hash("006-D8945-22.fnt"), watch.stage(korean, dry_run=True)["status"])
```

| Module | What it does |
|---|---|
| `codec` | Garmin's 2-bit bitmap `.fnt`, plain and "Compressed" (per-glyph RLE): `decode` (a `Decoded`: `header` with named fields such as `height` and `baseline`, `footer`, `cmap`, a short repr), `encode`, byte for byte |
| `truetype` | Garmin's masked TrueType `.bin` |
| `catalog` | `fetch_stock(part)`: pinned parts from `GFG_STOCK_DIR`/`GFG_CATALOG_DIR`, the cache (`~/.cache/georgian-garmin/catalog`) or their pinned URL, checked by SHA-256; other parts from Garmin's update catalog, their kind (bitmap or TrueType) read from the bytes. `GFG_OFFLINE=1` (as in the flake check) never downloads |
| `hosts` | The registry's host families |
| `build` | `add_georgian(font, size_like=…, markers=…)`, deterministic in the pinned environment, with the weight from the registry or an unpinned part's catalog name; `build` writes the asset folder |
| `render` | `render(fonts, text, replace=[…], family=…)`: a PNG and a per-character trace from the fallback models; `counts()` splits confirmed characters into drawn glyphs and boxes. `render_report(fixture, builds=…)` takes only what install would write there and lists the rest in `not_installed`; `replace=` renders any font |
| `watch` | `Watch()`: a `georgian-for-garmin rpc` session with the core's operations as methods; errors are `WatchError` with the core's code; Ctrl-C cancels the running operation |

The same as commands, each with `--json`, `--input <file|->` and examples in `--help`
(`--help --json` prints them as MCP tools):

```sh
python -m garmin_fonts fetch 006-D8448-70 --out stock
python -m garmin_fonts build 006-D8947-22 --markers ჰ=გ ก=ข --out builds
python -m garmin_fonts render --report ../crates/core/tests/reports/issue-12.json --with-our-fonts --out venu.png
python -m garmin_fonts roundtrip 006-D8448-70 006-D9035-03
python -m garmin_fonts build --out assets          # the asset folder, as nix build .#fonts
```

`builder/tests/test_watch.py` drives the `Watch` client against a stand-in for the rpc process.
With `GEORGIAN_FOR_GARMIN_CLI` set to a dev build of the CLI, it also checks the real one with
calls the core refuses before it touches a watch, and `test_rpc_contract.py` checks every request
a `Watch` method builds against the input schema the CLI declares (`--help --json`), and that
every declared operation has a method. CI runs both against its dev build:

```sh
cargo build -p georgian-for-garmin --features dev
cd builder && GEORGIAN_FOR_GARMIN_CLI=$PWD/../target/debug/georgian-for-garmin \
  pytest tests/test_rpc_contract.py tests/test_watch.py
```

## Rust

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p georgian-for-garmin -- inspect
cargo test -p georgian-for-garmin-core -p georgian-for-garmin --features dev   # the dev operations
```

The core's tests run against an in-memory watch; no hardware is needed. To trace a real watch,
set `GEORGIAN_FOR_GARMIN_TRACE=1`. `tests/support/fake.rs` in `crates/core` is a Venu 2 that can
be unplugged and plugged back in; the session, `rpc` and `mcp` tests share it.

Every operation is declared once in `crates/core/src/ops.rs`: JSON Schemas for its input and
output, from the Rust types, and its read-only, destructive, idempotent and open-world hints. The
one-shot commands, `rpc`, `mcp`, `--help --json` and the app all go through those declarations;
a new operation needs its declaration, its branch in `crates/core/src/session.rs`, and a subcommand in
`crates/cli`. The `rpc` and `mcp` protocols are described in
[Command line](command-line.md#sessions-rpc-and-mcp).

Which part a file in `ExtData` belongs to, and whether it is a waiting `.tmp`, an imported
`.fnt` or a TrueType `.bin`, comes from one function, `names::resolve` in
`crates/core/src/names.rs`; it takes the part number (`006-D9035-02.tmp`) and Garmin's short form
(`D903502.tmp`) alike. Every check before a write and every state `inspect` reports goes through
it; don't match file names anywhere else. The name install and restore write a part under comes
from one function too, `Font::file_name` in `crates/core/src/fonts.rs`: the `FileName` the watch
declares for the part in `GarminDevice.xml` (the Lily 2 Active's short `D894567.tmp`, the Venu
2's `006-D9035-03.tmp`), or the part number when it declares none that resolves to the part.

The cargo feature `dev` adds the development operations `read_file`, `stage_file` and
`remove_staged` (`crates/core/src/dev.rs`): `cargo build -p georgian-for-garmin --features dev`.
They write files of any content to a watch, so the feature only builds in debug: `--release`
with it fails to compile, and the release scripts never enable it. The CLI tests check that a
build without it lists none of them.

## Desktop app

```sh
cd crates/app
cargo tauri dev                                            # against a real watch
cargo tauri build --debug --features demo --bundles app    # a simulated watch, no hardware
```

The demo build walks through every support status (a tested Forerunner 165 and Venu 2, an
expected Venu 3 listing parts from two packs with a conflict, an unsupported fenix 6S Pro with
Restore, and an unsupported Venu), and a Forerunner 245 mounted as a USB
drive. `GEORGIAN_FOR_GARMIN_DEMO_WATCH=Venu` (any of those names) attaches that watch first;
`GEORGIAN_FOR_GARMIN_DEMO_WATCH=none` never finds a watch, so the waiting screen stays and shows
its help after 10 seconds. `GEORGIAN_FOR_GARMIN_DEMO_BUSY=macos_camera_service` (or
`garmin_express`, `desktop_mtp`, `other`, `unknown`) finds the watch held by such a program on
every connection. `GEORGIAN_FOR_GARMIN_DEMO_PLATFORM=windows` (or `macos`, `linux`) shows that
platform's words.
Every string is in `ui/strings.json`, with `ka` and `en` tables that must have the same keys; the
tests check that.

`tests/replay.rs` in `crates/core` replays every watch report posted in the issues against a fake
watch; add a new report with `scripts/report_fixture.py` ([fixtures](../crates/core/tests/reports/README.md)).

## Release

Releases are built with each platform's own toolchain and rustup's Rust, outside `nix develop`, so
the binaries don't depend on Nix. Nix only builds the fonts.

```sh
scripts/build-macos-release.sh     # needs Xcode command-line tools, rustup and Nix
scripts/build-linux-release.sh     # needs rustup, Nix, libwebkit2gtk-4.1-dev librsvg2-dev xdg-utils file
```

The macOS build writes a universal app and CLI (Apple silicon and Intel, macOS 11 or later) to
`target/macos-release/dist/`, with the fonts' manifest and `SHA256SUMS`. It checks the binaries'
linked libraries, architectures, minimum macOS version, ad-hoc signatures and embedded fonts. The
Linux build writes the static CLI with the udev rule, a `.deb` that installs the rule, and an
AppImage for the machine's architecture to `target/linux-release/dist/`.

Nix doesn't run on Windows: build the fonts elsewhere with `nix build .#fonts`, copy the folder
over, and run `scripts/build-windows-release.ps1 -Fonts <dir> [-Arch x64|arm64]`. It needs Visual
Studio 2022 or later with the C++ x64 and ARM64 build tools, and rustup. It writes the CLI zips and
per-user NSIS installers to `target\windows-release\dist\`.

To publish, bump the workspace version, update `CHANGELOG.md`, and push a `v<version>` tag.
`release.yml` builds every platform, each architecture on its own runner, and creates a draft
release with one `SHA256SUMS`. Tag builds always start from scratch; manual runs can skip macOS
and use a Rust build cache.
