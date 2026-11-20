<img src="crates/app/icons/icon.png" alt="Georgian for Garmin logo" width="96" align="right">

# Georgian for Garmin

ქართული შრიფტი Garmin-ის საათებისთვის.

Garmin watches show Georgian text in notifications as boxes. This project adds Georgian letters to
a font the watch already uses, so notifications from your phone show `მხედრული` and `ᲛᲗᲐᲕᲠᲣᲚᲘ`
instead of `□□□□□□□□`.

<img src="docs/venu-2-georgian-notification.jpg" alt="Garmin Venu 2 showing a Gmail notification in Georgian" width="360">

*A Gmail notification on a Venu 2 (firmware 19.05).*

Unofficial; not affiliated with or endorsed by Garmin.

## How it works

Garmin watches load optional fonts for other scripts, such as Armenian, from a hidden folder. This
project takes Garmin's own Armenian font in each size, adds all 66 modern Georgian letters
(Mkhedruli and Mtavruli) and copies it to the watch over USB. The watch loads it after a
restart. Nothing else on the watch changes: no firmware update, and Armenian still works. Garmin's
original font can be copied back the same way; on the Venu 2 that restored the boxes. Details:
[How it works](docs/how-it-works.md).

![Garmin's Armenian letters and the Georgian added to the same 27 px font, from their 2-bit bitmaps](docs/georgian-glyphs-27px.png)

Georgian is drawn at the size of the font's own letters: in the 27 px font, Georgian lowercase is
19 px tall, like the Armenian ([Size](fonts/README.md#size)).

## Supported watches

| Watch | Software | Status |
|---|---|---|
| Venu 2 | 19.05 | Tested: Georgian in pop-ups, the notification list and opened notifications |

The app reads the font list of the connected watch and adds Georgian to every Garmin Armenian
font it uses, in any size: NotoSans Regular, NotoSans Armenian Regular and NotoSans Armenian
Bold. Other models and software versions are untested until someone confirms
them. If yours isn't listed, use **Share my watch's font list** in the app (or
`georgian-for-garmin inspect --report`) to open an issue, and say whether Georgian appeared.

## Install

You need your watch's USB data cable. Download the app for your computer from
[Releases](https://github.com/lubaretsi/georgian-for-garmin/releases):

| Computer | Download, then |
|---|---|
| Mac, macOS 11 or later | `Georgian-for-Garmin-<version>-macos.zip`: unzip it and open **Georgian for Garmin** |
| Windows 10 or 11 | `Georgian-for-Garmin-<version>-windows-x64-setup.exe` (`-arm64-` on Arm PCs): run it |
| Debian, Ubuntu | `Georgian-for-Garmin-<version>-linux-x86_64.deb` (`-aarch64` on Arm): `sudo apt install ./Georgian-for-Garmin-*.deb` |
| Other Linux | `Georgian-for-Garmin-<version>-linux-x86_64.AppImage`: `chmod +x` it, run it, and [install the udev rule](docs/command-line.md#linux) |

The app isn't signed, so the first time:

- **macOS** blocks it: open **System Settings → Privacy & Security** and click **Open Anyway**.
- **Windows** says "Windows protected your PC": click **More info → Run anyway**.

Then:

1. Quit Garmin Express if it's running, and connect the watch. On Linux,
   [release it from GNOME or KDE](docs/command-line.md#linux) first.
2. Click **Install Georgian** (**ქართულის დაყენება**).
3. Unplug the watch and restart it: hold the top button, choose **Power Off**, then turn it on.
4. Check it: send yourself `მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ`.

To undo, connect the watch, click **Restore original font**, then unplug and restart it again.

The app is in Georgian, with an English switch in the top corner. The macOS app is tested with a
watch. On Windows (11 on Arm) and Linux (Ubuntu 24.04) the command-line installer is tested with a
watch; the Windows and Linux apps are tested only without one so far. The Linux app needs glibc
2.34 or newer.

## Command line

The same installer runs from a terminal. Download `georgian-for-garmin-<version>-` plus
`macos-universal.tar.gz`, `windows-x64.zip`, `windows-arm64.zip`, `linux-x86_64.tar.gz` or
`linux-aarch64.tar.gz` from Releases. The Linux archives include the udev rule.

```sh
georgian-for-garmin inspect          # watch model, font pack, tested status, staged files
georgian-for-garmin install          # stage Georgian in every Armenian font the watch lists
georgian-for-garmin restore-stock    # stage Garmin's original font
```

Every command takes `--json` for scripts. See [Command line](docs/command-line.md).

## Install with an AI agent

To have an AI agent do it without this project's app, give it this prompt:

```text
Follow https://github.com/lubaretsi/georgian-for-garmin/blob/main/docs/install-with-an-agent.md
to add Georgian to my Garmin watch's notifications.
```

## More

- [How it works](docs/how-it-works.md): the watch's font folder, the font format, what was tested.
- [Command line](docs/command-line.md): commands, JSON output, errors and troubleshooting.
- [Development](docs/development.md): building the fonts, the app and a release from source.
- [Font registry](fonts/README.md): adding a watch or a font pack.

## License

The code is [MIT](LICENSE). The Georgian letters are drawn from
[Noto Sans Georgian](https://fonts.google.com/noto/specimen/Noto+Sans+Georgian), under the SIL Open
Font License 1.1, included with each release.
