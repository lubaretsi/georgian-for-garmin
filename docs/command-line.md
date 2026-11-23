# Command line

`georgian-for-garmin` does what the app does, from a terminal or a script.

## Install

Download the archive for your computer from
[Releases](https://github.com/lubaretsi/georgian-for-garmin/releases). The fonts are built in; it
needs no other libraries, no driver and no internet connection.

### macOS

Unpack `georgian-for-garmin-macos-universal.tar.gz` and clear the download flag macOS sets so the
binary can run:

```sh
tar xzf georgian-for-garmin-macos-universal.tar.gz
cd georgian-for-garmin-macos-universal
xattr -d com.apple.quarantine georgian-for-garmin
./georgian-for-garmin --version
```

It runs on macOS 11 or later, Apple silicon and Intel.

### Windows

Unzip `georgian-for-garmin-windows-x64.zip` (`-arm64` on Arm PCs) and run
`georgian-for-garmin.exe` from its folder in PowerShell or Command Prompt. It uses Windows' own
MTP support, so there's no driver to install. It isn't signed: if Windows blocks it, click
**More info → Run anyway**, or clear the download flag with
`Unblock-File .\georgian-for-garmin.exe` in PowerShell. Tested with a Venu 2 on Windows 11 on Arm,
the x64 and Arm64 builds.

### Linux

Unpack `georgian-for-garmin-linux-x86_64.tar.gz` (`-aarch64` on Arm). The binary is
static and runs on any distribution; the folder also holds `70-georgian-for-garmin.rules`. It was
tested with a Venu 2 on Ubuntu 24.04 (arm64). Two setup steps, for the command line, the
AppImage and the `.deb` alike (the `.deb` does the first one for you):

1. **Let your user open the watch.** Install the udev rule from the unpacked folder, then
   reconnect the watch:

   ```sh
   sudo install -m 644 70-georgian-for-garmin.rules /etc/udev/rules.d/
   sudo udevadm control --reload
   ```

   In a source checkout the rule is `packaging/linux/70-georgian-for-garmin.rules`.

   The user at the computer's screen needs nothing more. For SSH or other sessions without a
   screen, also run `sudo usermod -aG plugdev "$USER"` and log in again (on Fedora and Arch,
   create the group first: `sudo groupadd --system plugdev`).
2. **Release the watch from the desktop.** GNOME and KDE open MTP devices as soon as they're
   plugged in, and only one program can use one at a time:
   - **GNOME:** eject the watch in the Files sidebar, or run `gio mount -l` and then
     `gio mount -u <its mtp:// address>`.
   - **KDE:** run `kquitapp6 kiod6` (`kquitapp5 kiod5` on Plasma 5). KDE claims the watch again the
     next time it's plugged in.

   These two cases weren't tested on a real desktop.

## Commands

| Command | What it does |
|---|---|
| `inspect` | Shows the model, software version, its status in the support list with the GitHub issue to follow, the font packs the watch lists with each listed part and whether it was tested on this model and software, and which font files are waiting on the watch. Read-only. |
| `inspect --report` | Prints the watch report the app shares in a GitHub issue: model, part number, software, the font files the watch declares and holds, what this build would install, the app version and OS ([Reporting](reporting.md)). No serial number or unit ID. |
| `install` | Copies the Georgian font into every part of the bundled packs that the watch lists, across all packs, then reads each file back. |
| `restore-stock` | The same with Garmin's original fonts. |
| `inspect-drive` | Reads a watch that connects as a USB drive instead of MTP (`--path` its volume; default: the one mounted volume with `GARMIN/GarminDevice.xml`). Nothing installs this way; `--report` prints the same report. Read-only. |
| `issue-link` | Prints the GitHub link that opens the watch report form filled in, from `--input` `{"report": …, "outcome": …, "answers": …, "busy": …}` (`report` as `inspect --report --json` prints it; `busy` as a `device_busy` error gives it, alone for a watch that couldn't be read). Touches nothing. |
| `extdata-list` | Lists every file in the watch's hidden `GARMIN/ExtData` folder: the normal listing plus, on watches that answer Garmin's `GetObjectIds` (0x9002), the files it leaves out (on the Venu 2 the read-only fonts the watch imported), with size, `hidden` and `read_only`. `--hash 006-D9035-03.fnt,…` reads those files and adds their SHA-256s. Pages with `--offset` and `--limit` (default 200); `--output-file FILE` writes the whole list as JSON instead. Read-only. |
| `wait-for-watch` | Waits until a watch can be opened, e.g. after it was unplugged and restarted, checking twice a second for up to `--timeout-seconds` (default 60). `no_device` and `device_error` are waited out; any other error ends the wait. Read-only. |
| `rpc` | Serves every command as JSON-RPC 2.0 on stdin and stdout, keeping the watch open between calls ([below](#sessions-rpc-and-mcp)). |
| `mcp` | Serves every command as an MCP tool over stdio, with the official MCP SDK, keeping the watch open between calls. |

After `install` or `restore-stock`, unplug the watch and restart it; it loads the fonts on
restart. Quit Garmin Express first: it holds the watch's USB connection.

Options:

- `--parts 006-D9035-02,006-D9035-03`: only these part numbers (`install`, `restore-stock`).
  `inspect` lists the watch's parts. Part numbers, not pixel sizes, because packs share sizes
  (15, 21, 24 and 25 px each come in two packs).
- `--json`: print one JSON value on stdout; errors go to stderr as
  `{"error": {"code": "…", "message": "…"}}`, and `device_busy` adds `busy`
  ([below](#busy-watch)).
- `--input FILE` or `--input -`: take the input as a JSON object keyed like the command's input
  schema, e.g. `{"parts": ["006-D9035-03"]}`. Flags given too override it.
- `--help --json`: print every command's definition, with JSON Schemas for its input and output,
  as MCP tools.

Running `install` again after it succeeded changes nothing and reports `already_staged`.

## Sessions: rpc and mcp

`georgian-for-garmin rpc` and `georgian-for-garmin mcp` are long-lived: they open the watch at the
first call that needs it and keep it open until they end, so a program or an agent can run many
steps without reconnecting. While one runs, no other program (Garmin Express, the app, another
`georgian-for-garmin`) can use the watch. When the watch stops answering, as after an unplug, a
call fails with `no_device` (or `device_error`) and the next call opens it again;
`wait_for_watch` waits for it. Calls run one at a time, in the order they arrive. Both modes take
the operations and schemas `--help --json` prints.

`rpc` reads one JSON-RPC 2.0 message per line and writes one per line:

```sh
$ printf '%s\n' '{"jsonrpc": "2.0", "id": 1, "method": "extdata_list", "params": {"hash": ["006-D9035-03.fnt"]}}' \
    | georgian-for-garmin rpc
{"jsonrpc":"2.0","id":1,"result":{"extdata":{…},"files":[…],"total":…}}
```

- The method is the operation's name, `params` its input (an object; absent means `{}`), the
  result its output. `params._meta` is protocol metadata, removed before the input is checked.
- Errors: `{"code": …, "message": …, "data": {"code": "<error code>", "message": …}}`, with `busy`
  in `data` for `device_busy` and `rollback` after a failed write ([below](#failed-writes)).
  `code` is -32602 for `invalid_input`, -32800 for `cancelled`,
  -32000 for any other error code ([below](#exit-codes-and-errors)), -32601 for an unknown
  method, -32600 for a message that isn't a request (batches aren't supported) and -32700 for a
  line that isn't JSON; after any of them the session goes on.
- Progress: a request with `params._meta.progressToken` gets
  `{"jsonrpc": "2.0", "method": "notifications/progress", "params": {"progressToken": …, "progress": …, "total": …, "message": …}}`
  at each step: one per file for `install` and `restore_stock` (0 when every file was checked,
  then each file written), two for `stage_file`, the seconds waited for `wait_for_watch`.
- Cancellation: `{"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": …}}`
  stops a queued or running call. It answers with -32800 (`cancelled`). `install`,
  `restore_stock` and `stage_file` stop between files: before the first write nothing is written;
  after it, the call tries to undo what it wrote, as when a write fails, and the error's
  `rollback` says whether it could ([below](#failed-writes)). A single file transfer is never cut
  off.
- At the end of stdin, the calls already read still run and are answered, then `rpc` exits.

`mcp` speaks MCP over stdio: `tools/list` returns the same definitions, `tools/call` runs one
(an error is a result with `isError` and `{"error": {"code", "message"}}` as structured content).
Progress and cancellation use MCP's `notifications/progress` (with `_meta.progressToken`) and
`notifications/cancelled`, with the same steps as `rpc`. For example, in an MCP client's
configuration: `{"command": "georgian-for-garmin", "args": ["mcp"]}`.

### Development operations

A development build (`cargo build -p georgian-for-garmin --features dev`, debug only; release
builds never have them, and `--release` with the feature fails to compile) adds commands for
experiments with fonts this build doesn't install:

| Command | What it does |
|---|---|
| `read-file --name D844870.fnt --path out.fnt` | Copies one ExtData file, from either listing, to a new local file and prints its SHA-256. Never overwrites a local file. |
| `stage-file --source font.tmp --name D844870.tmp` | Writes a local file as `GARMIN/ExtData/<name>` and reads it back by SHA-256. `name` is `<part>.tmp` for a part the watch declares under `Garmin/ExtData`, as `006-D8448-70.tmp` or its short form `D844870.tmp`. Like `install`, it checks every file for that part first: a file under `name` is replaced only when it is identical (`already_staged`), Garmin's original or one of this program's Georgian builds (by content), one this tool staged, or has a SHA-256 given with `--garmin-sha256` (Garmin's file, from a pinned catalog); an imported `<part>.fnt` must be one of those too, and another waiting `.tmp` for the part is a `conflict`. `--dry-run` checks and writes nothing. Prints an id. |
| `remove-staged --id stg-…` | Deletes the file staged (or created by `mtp-create`) under that id, only while the watch's file under its name still has the logged SHA-256 (otherwise `conflict`); `not_found` once it's gone (imported at a restart, for example), `already_removed` when this tool removed it before. |

`stage-file` and `mtp-create` log each id with the file's name and SHA-256 in `staged.jsonl`, in
`GEORGIAN_FOR_GARMIN_STATE_DIR` or the per-user state folder (`~/Library/Application Support/georgian-for-garmin`
on macOS, `%LOCALAPPDATA%\georgian-for-garmin` on Windows, `$XDG_STATE_HOME/georgian-for-garmin`
or `~/.local/state/georgian-for-garmin` elsewhere), so an id from an `rpc` session works in a
later one-shot `remove-staged`.

It also adds the raw MTP operations, for debugging from a script over `rpc` or `mcp`:
`mtp-device-info`, `mtp-storage-ids`, `mtp-storage-info`, `mtp-handles`, `mtp-object-info`,
`mtp-read`, a read-only `ptp-transaction`, the checked writes `mtp-create` and `mtp-delete`
(each needs `--allow-write`), and `mtp_trace` (sessions only), which sends every PTP
transaction as `notifications/trace`. They print JSON. What each does and the checks are in
[Development](development.md#debugging-with-raw-mtp).

## JSON output

`inspect --json` returns:

- `device`: `model`, `part_number`, `software_version`;
- `supported`: the watch lists a bundled part this build installs and accepts the needed MTP
  writes; `restorable`: `restore-stock` has parts to write, including packs this build only
  restores (Swiss 721 Hebrew);
- `state`: Georgian on the watch, from the files of the parts `install` writes: `installed`
  (also a bitmap part imported at the last restart), `waiting` (bitmap, until the restart),
  `partly`, `beta` (an earlier release's build), `not_installed`, `not_visible` (the watch shows
  its `.bin` files in neither listing), `stock_waiting`, `other`, `unknown` or `unsupported`;
- `tested`: `status` for the whole install and `verified_software`, the software versions this
  model was verified on with any of its parts. `status` is `tested` when every target part is,
  `tested_other_software` when every part was verified on this model but some only on other
  software, and `untested` when any part (or every part) wasn't verified on this model;
- `support`: the watch's status in the support list ([How it works](how-it-works.md)):
  `status` (`tested`, `expected`, `unsupported` or `unknown`), `listed` (the model's part
  number is in the list), `issue` (the GitHub issue to follow, or null), `reason` (for
  `unsupported`: `en` and `ka` sentences, else null), `prediction` (`noto_truetype`,
  `bitmap_armenian` or `bitmap_korean` for a model the list only predicts to work, else null; it
  changes neither the status nor `report_wanted`) and `report_wanted` (a report from this watch
  adds information). `inspect-drive --json` has it too;
- `hosts`: which kind of font gets Georgian on this watch: `kind` (`bitmap` when it lists any
  bundled bitmap part, otherwise `truetype`; null when it lists no bundled part), `reason` (that
  choice in a sentence), `skipped_parts` (parts of the other kind it lists, left alone; the
  fenix 7 Pro's Hebrew TrueType fonts, for example) and `restore_only_parts` (parts of packs this
  build only restores);
- `packs`: every bundled pack of that kind the watch lists, in registry order, with `id`, `name`,
  `kind`, `weight` and `parts`: the parts the watch lists, each with `size_px` (null for
  TrueType parts, which scale) and its own `tested` status;
- `fonts`: one entry per part, with `kind`, `size_px`, whether the watch lists it, whether
  `install` writes it (`target`), whether `restore-stock` does (`restore`), `file` (what the
  part's file in the folder holds, `<part>.tmp` for bitmap parts and `<part>.bin` for TrueType
  parts: `none`, `georgian`, `georgian_beta`, `stock`, `other`, `not_visible` for a TrueType part
  the watch declares but doesn't show, or `unknown` without the folder) and `imported` (`file`
  is from the hidden listing: the `<part>.fnt` the watch imported a bitmap part's `.tmp` as at
  its last restart, or a TrueType part's hidden `.bin`);
- `extdata`: the hidden folder's handle, how it was found, and every file in it with its `name`,
  `handle` and size in `bytes`. `sha256` is the file's SHA-256 only for a file named for a
  bundled part with the size of its original or Georgian version; it is `null` for every other
  file, which `inspect` doesn't read (watches with TrueType fonts keep files of several MB
  there);
- `fonts[].mark`: in beta builds, the test-mark variant (`A`, `B`, …) drawn into the part's
  Georgian font ([Reporting](reporting.md#beta-test-marks)); null otherwise;
- `report`: the watch report as an object (`--report --json` prints it): only allowlisted
  fields, listed in [Reporting](reporting.md#whats-in-a-report), with `text`, what `--report`
  prints.

`install --json` and `restore-stock --json` return an overall `status` (`staged_verified` or
`already_staged`) and `packs`: each pack written to, with its `kind` and its parts' paths on the
watch, sizes in bytes, SHA-256s and statuses. A bitmap part is staged as a new `.tmp` under the
file name the watch declares for it (`006-D9035-03.tmp`, or the short form, as `D894567.tmp`, on
a watch that declares that); a TrueType part replaces the watch's `<part>.bin`, and a failed run
tries to write the original back ([Failed writes](#failed-writes)).

## Exit codes and errors

Exit code 0 means success, 2 invalid input, 1 anything else.

| Code | Meaning | What to do |
|---|---|---|
| `no_device` | No Garmin watch in USB file-transfer mode | Connect it with a data cable; try another port or cable. |
| `device_busy` | Another program holds the watch's USB connection | The message names it when the system tells ([below](#busy-watch)) and says how to free it on this system. Then try again. |
| `permission_denied` | This user may not open the watch's USB device | On Linux, install the udev rule (above); the message also gives a one-line fix. |
| `multiple_devices` | More than one Garmin device is connected | Leave only the watch connected. |
| `unsupported_device` | The watch lists none of the bundled font packs, or not a requested part, or lists a TrueType part whose file can't be read at another version or file name than the bundled one (perhaps a newer Garmin font) | Share `inspect --report` in an issue. |
| `conflict` | A part's file, or the font the watch imported for it (a bitmap part's `.fnt`, which the next restart would replace), in either listing and under either name form (`006-D9035-02.tmp` or Garmin's short `D903502.tmp`), is neither Garmin's original nor one of this program's builds, by content, or the part has several such files (the error names them); nothing was written | For a waiting `.tmp`, unplug and restart the watch so it loads that file, then run again. Otherwise share `inspect --report` in an issue. |
| `restore_first` | Install: a TrueType file the watch doesn't let be read is declared below 1.00, as watches declared this program's builds before 0.3.0; install never writes over a file it can't identify, so nothing was written | Run `restore-stock`, restart the watch, then install again. |
| `verification_failed` | A copied file read back differently; it was deleted, and a replaced `.bin` written back, where that worked (`rollback`, [below](#failed-writes)) | Run again. |
| `extdata_not_found` | The hidden font folder wasn't found; nothing was written | Restart the watch, reconnect, run again. |
| `garmin_folder_not_found` | The watch's `GARMIN` folder or `GarminDevice.xml` couldn't be read | Reconnect and run again. |
| `device_error` | A USB transfer or MTP command failed | Check the cable and run again. The message names the step that failed and whether a file was left on the watch. |
| `font_unavailable` | This build doesn't include a requested part | Check the part number, or download the release again. |
| `invalid_input` | The input doesn't match the command's schema | Check the arguments. |
| `internal` | A bug | Please open an issue. |
| `cancelled` | `rpc` or `mcp`: the client cancelled the call; what it had written was undone where possible (`rollback`) | Nothing; run it again when wanted. |

### Failed writes

When `install`, `restore-stock` or `stage-file` fails or is cancelled after writing, it tries to
undo its changes: files it created are deleted, files it replaced are written back from the bytes
read before. The error then carries `rollback`:

```json
{"error": {"code": "device_error", "message": "…", "rollback": {"status": "incomplete", "files": [
  {"path": "GARMIN/ExtData/006-D9035-02.tmp", "action": "delete", "done": false,
   "result": "GARMIN/ExtData/006-D9035-02.tmp (0x01000101) could not be deleted (…)"}]}}}
```

`status` is `complete` when every step worked, `incomplete` when one didn't (a delete the watch
refused, a disconnect in the middle); `action` is `delete` or `restore`. After an incomplete
rollback the message says so and names the files: connect the watch, run Restore original font
(`restore-stock`), and restart the watch. The app says the same.

### Busy watch

`device_busy` names what holds the watch when the system tells, and never stops or signals it:

```json
{"error": {"code": "device_busy", "message": "…", "busy": {
  "holder": {"kind": "macos_camera_service", "name": "ptpcamerad", "pid": 512, "confirmed": true},
  "product_id": 20087, "product_name": null,
  "interface": {"class": 6, "subclass": 1, "protocol": 1}}}}
```

- `holder.kind`: `garmin_express`, `macos_camera_service` (`ptpcamerad`, which takes still-image
  devices for Photos and Image Capture), `desktop_mtp` (GNOME's `gvfsd-mtp`, KDE's `kiod`,
  Android File Transfer, OpenMTP, MacDroid…), `other` (a program none of these lists knows) or
  `unknown`. The known names are listed in one place, `crates/core/src/busy.rs`.
- macOS: the IOKit registry's `UsbExclusiveOwner` on the watch's `IOUSBHostInterface` (and its
  `IOUSBHostDevice`), `"pid 1234, name"`, read with `ioreg`; any user can read it. Process names
  there are cut to 16 bytes. `product_name` is the device's own `USB Product Name`, null for a
  watch without a product string, as the Venu 2.
- Linux: the process with the watch's `/dev/bus/usb/BBB/DDD` open, from `/proc/*/fd` (the user's
  own processes).
- Windows doesn't say who holds a device: the holder is Garmin Express when it is running
  (`tasklist`), with `confirmed` false, and otherwise `unknown`; `product_id` and `interface`
  are null.

The message says how to free the watch on this system. For macOS's camera service: quit Photos,
Image Capture, Preview and other photo importers, then unplug and reconnect the watch; if it is
still busy, share the error in an issue so we can look. No Garmin watch has been seen held by it
yet.

## Troubleshooting

- **The watch froze during a copy.** The next run recovers it automatically, with a USB reset if
  needed, and reports whether a partial file was left.
- **Still boxes after a restart.** Run `inspect`: if files are still waiting, the watch hasn't
  restarted since they were copied.
- **To see what's sent over USB,** set `GEORGIAN_FOR_GARMIN_TRACE=1`. Each MTP message and USB
  transfer goes to stderr.
