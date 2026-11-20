# Command line

`georgian-for-garmin` does what the app does, from a terminal or a script.

## Install

Download `georgian-for-garmin-<version>-macos-universal.tar.gz` from
[Releases](https://github.com/lubaretsi/georgian-for-garmin/releases), unpack it, and clear the
download flag macOS sets so the binary can run:

```sh
tar xzf georgian-for-garmin-*-macos-universal.tar.gz
xattr -d com.apple.quarantine georgian-for-garmin
./georgian-for-garmin --version
```

It runs on macOS 11 or later, Apple silicon and Intel. The fonts are built in; it needs no other
libraries and no internet connection.

### Linux

There's no Linux download yet; build it from source ([Development](development.md)). It was
tested with a Venu 2 on Ubuntu 24.04 (arm64). Two setup steps:

1. **Let your user open the watch.** Install the udev rule, then reconnect the watch:

   ```sh
   sudo install -m 644 packaging/linux/70-georgian-for-garmin.rules /etc/udev/rules.d/
   sudo udevadm control --reload
   ```

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
| `inspect` | Shows the model, software version, font pack, whether this model and software were tested, and which font files are waiting on the watch. Read-only. |
| `inspect --report` | Prints the watch's font list as Markdown for a GitHub issue: model, part number, software and font files. No serial number or unit ID. |
| `install` | Copies the Georgian font in every size of the watch's font pack, then reads each file back. |
| `restore-stock` | The same with Garmin's original font. |

After `install` or `restore-stock`, unplug the watch and restart it; it loads the fonts on
restart. Quit Garmin Express first: it holds the watch's USB connection.

Options:

- `--sizes-px 24,27`: only these sizes (`install`, `restore-stock`).
- `--json`: print one JSON value on stdout; errors go to stderr as
  `{"error": {"code": "…", "message": "…"}}`.
- `--input FILE` or `--input -`: take the input as a JSON object keyed like the command's input
  schema, e.g. `{"sizes_px": [27]}`. Flags given too override it.
- `--help --json`: print every command's definition, with JSON Schemas for its input and output,
  as MCP tools.

Running `install` again after it succeeded changes nothing and reports `already_staged`.

## JSON output

`inspect --json` returns:

- `device`: `model`, `part_number`, `software_version`;
- `supported`, and `pack`: the selected font pack, its sizes, and `tested` (`status` is `tested`,
  `tested_other_software` or `untested`, with the verified parts and software versions);
- `fonts`: one entry per part, with `size_px`, whether the watch lists it, and `staged` (what's
  waiting in the folder: `none`, `georgian`, `stock`, `other`, or `unknown` without the folder);
- `extdata`: the hidden folder's handle, how it was found, and the files in it with their SHA-256;
- `report`: the same data as `--report`.

`install --json` and `restore-stock --json` return an overall `status` (`staged_verified` or
`already_staged`), the pack, and each part with its path on the watch, size in bytes and SHA-256.

## Exit codes and errors

Exit code 0 means success, 2 invalid input, 1 anything else.

| Code | Meaning | What to do |
|---|---|---|
| `no_device` | No Garmin watch in USB file-transfer mode | Connect it with a data cable; try another port or cable. |
| `device_busy` | Another program holds the watch's USB connection | Quit Garmin Express, then try again. |
| `multiple_devices` | More than one Garmin device is connected | Leave only the watch connected. |
| `unsupported_device` | The watch lists none of the bundled font packs, or not the requested size | Share `inspect --report` in an issue. |
| `conflict` | A different file is already waiting on the watch; nothing was written | Unplug and restart the watch so it loads that file, then run again. |
| `verification_failed` | A copied file read back differently; it was deleted | Run again. |
| `extdata_not_found` | The hidden font folder wasn't found; nothing was written | Restart the watch, reconnect, run again. |
| `garmin_folder_not_found` | The watch's `GARMIN` folder or `GarminDevice.xml` couldn't be read | Reconnect and run again. |
| `device_error` | A USB transfer or MTP command failed | Check the cable and run again. The message names the step that failed and whether a file was left on the watch. |
| `font_unavailable` | This build doesn't include the requested size | Download the release again. |
| `invalid_input` | The input doesn't match the command's schema | Check the arguments. |
| `internal` | A bug | Please open an issue. |

## Troubleshooting

- **The watch froze during a copy.** The next run recovers it automatically, with a USB reset if
  needed, and reports whether a partial file was left.
- **Still boxes after a restart.** Run `inspect`: if files are still waiting, the watch hasn't
  restarted since they were copied.
- **To see what's sent over USB,** set `GEORGIAN_FOR_GARMIN_TRACE=1`. Each MTP message and USB
  transfer goes to stderr.
