# Development

## Layout

| Path | What it is |
|---|---|
| `fonts/registry.json` | Garmin font packs that get Georgian: parts, download URLs, SHA-256s, tested watches, and the support list of every known watch model ([how to add one](../fonts/README.md)) |
| `builder/` | Python font builder: decodes Garmin's font format, adds Georgian, writes the asset folder |
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
pinned hashes only after testing the new fonts on a watch.

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
it; don't match file names anywhere else.

The cargo feature `dev` adds the development operations `read_file`, `stage_file` and
`remove_staged` (`crates/core/src/dev.rs`) and the raw MTP operations below
(`crates/core/src/raw.rs`): `cargo build -p georgian-for-garmin --features dev`. They write files
of any content to a watch, so the feature only builds in debug: `--release` with it fails to
compile, and the release scripts never enable it. The CLI and session tests check that a build
without it lists none of them.

### Debugging with raw MTP

The raw MTP operations let an experiment be a script over `rpc` or `mcp` instead of a new
operation. They mirror the core's `Mtp` trait (`crates/core/src/mtp.rs`), so they work over USB
(macOS, Linux) and Windows Portable Devices alike, except `ptp_transaction`. Handles, storage IDs
and codes are plain integers in and out; the CLI also takes `0x…`.

| Operation | Input | What it does |
|---|---|---|
| `mtp_device_info` | none | The GetDeviceInfo dataset the session read when it opened. |
| `mtp_storage_ids` | none | GetStorageIDs. |
| `mtp_storage_info` | `storage` | GetStorageInfo: type, capacity, free space, description. |
| `mtp_handles` | `storage`, `parent`, `hidden` | GetObjectHandles (`parent` 0xFFFFFFFF: the root; 0: every object), or with `hidden` Garmin's GetObjectIds (0x9002), which also lists the children GetObjectHandles leaves out, such as imported fonts. |
| `mtp_object_info` | `handle` | GetObjectInfo, and whether the object is read-only. |
| `mtp_read` | `handle`, `offset`, `length`, `path`, `max_bytes` | The object's bytes, or a range: GetPartialObject when the watch lists it, else the whole object cut to the range. Base64 up to `max_bytes` (default 1 MiB), or a new local file at `path`. |
| `ptp_transaction` | `opcode`, `params` (up to 5), `path`, `max_bytes` | One transaction with no data-out phase: the response code and its name (a code other than OK is an answer, not an error), the response parameters and the data in. |
| `mtp_create` | `name`, `data_base64` or `source`, `storage`, `parent`, `allow_write` | One new file in `GARMIN/ExtData`, read back by SHA-256. |
| `mtp_delete` | `handle`, `allow_write` | Deletes one file in `GARMIN/ExtData`, for good. |
| `mtp_trace` | `enabled` | Turns the session's live trace on or off. |

The checks, with nothing to override them (AGENTS.md: nothing is written before every target is
checked):

- The reads, `ptp_transaction` and `mtp_trace` change nothing on the watch.
- `ptp_transaction` sends only reads: GetDeviceInfo, GetStorageIDs, GetStorageInfo,
  GetNumObjects, GetObjectHandles, GetObjectInfo, GetObject, GetThumb, GetPartialObject,
  GetDevicePropDesc, GetDevicePropValue, MTP's GetObjectPropsSupported, GetObjectPropDesc,
  GetObjectPropValue, GetObjectPropList and GetObjectReferences, and Garmin's 0x9002. Any other
  code, unknown vendor codes included, is refused as `invalid_input` before anything is sent, with
  the reason: writes go through `mtp_create` and `mtp_delete`; OpenSession and CloseSession belong
  to the session; FormatStore, ResetDevice and PowerDown are never sent. A vendor operation that
  proves useful gets its own checked operation later.
- `mtp_create` and `mtp_delete` are the only writes. Each needs `allow_write: true` (else
  `invalid_input`, before the watch is opened) and makes `stage_file`'s checks, sharing its code
  (`Identify` and `write_target` in `dev.rs`). `mtp_create` writes only into `GARMIN/ExtData`
  (`storage` and `parent`, if given, must be its), only a file of a part the watch declares
  (`<part>.tmp`, `.fnt` or `.bin`, either name form), never under a name that is there already
  or beside a file of the same kind in the normal listing (`mtp_delete` that first), and only
  when every other file of the part, in either listing, is Garmin's original or one of this
  program's Georgian builds (by this build's fonts), or a file this tool staged or created (by
  `staged.jsonl`). `mtp_delete` deletes only a file of `ExtData`'s normal listing identified the
  same way: never a hidden or read-only import, an unidentified file or anything outside
  `ExtData`.
- Every write goes into `staged.jsonl`: `mtp_create` logs its id, name, part and SHA-256 before
  writing and the created handle after, so `remove_staged` deletes it later; `mtp_delete` logs
  the deleted file's name, handle, SHA-256 and what it was identified as. Nothing undoes a delete;
  `remove_staged` refuses its id.

While the trace is on, every PTP transaction of every call is sent before the call's result as
`notifications/trace` in both `rpc` and `mcp` (in `mcp` a custom notification, since MCP's
logging is deprecated): `opcode`, `operation` (its name, or null), `params`, `response_code`,
`response`, `response_params`, `data_out_bytes`, `data_in_bytes`, `duration_ms`, `error` (a
failed transfer), and the call's `requestId`. It includes the GetStorageIDs the session sends
before each call to check that the watch still answers. It reuses the stderr trace's module,
`crates/core/src/trace.rs`; `GEORGIAN_FOR_GARMIN_TRACE=1` still prints the containers and USB
transfers.

On Windows, `ptp_transaction` answers `unsupported_device` ("ptp_transaction needs the USB
backend; Windows uses WPD"): WPD sends its own transactions. The trace there shows only the MTP
commands this program sends through WPD's MTP extension (GetDeviceInfo, GetObjectInfo, Garmin's
0x9002, GetStorageInfo), not the ones WPD makes for its own calls.

Each has a CLI subcommand (`mtp-read`, `ptp-transaction`, …) that prints JSON, except
`mtp_trace`: a one-shot call has no session to trace.

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
