# Georgian for Garmin

ქართული შრიფტი Garmin-ის საათებისთვის.

Garmin watches show Georgian text in notifications as boxes. This project adds Georgian letters to
a font the watch already uses, so notifications from your phone show `ქართული` and `ᲥᲐᲠᲗᲣᲚᲘ`
instead of `□□□□□□□`.

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

## Supported watches

| Watch | Software | Status |
|---|---|---|
| Venu 2 | 19.05 | Tested: Georgian in pop-ups, the notification list and opened notifications |

The app reads the font list of the connected watch and adds Georgian to every Garmin Armenian
font it uses: NotoSans Regular, or NotoSans Armenian Regular or Bold, in any size. Other models
and software versions that use them are untested until someone confirms them. If yours isn't
listed, use **Share my watch's font list** in the app (or `georgian-for-garmin inspect --report`)
to open an issue, and say whether Georgian appeared.

## Install

You need a Mac with macOS 11 or later and your watch's USB data cable.

1. Download `Georgian-for-Garmin-<version>-macos.zip` from
   [Releases](https://github.com/lubaretsi/georgian-for-garmin/releases) and unzip it.
2. Open **Georgian for Garmin**. The app isn't notarized by Apple, so macOS blocks it the first
   time: open **System Settings → Privacy & Security** and click **Open Anyway**.
3. Quit Garmin Express if it's running, and connect the watch.
4. Click **Install Georgian** (**ქართულის დაყენება**).
5. Unplug the watch and restart it: hold the top button, choose **Power Off**, then turn it on.
6. Check it: send yourself `ქართული ᲥᲐᲠᲗᲣᲚᲘ`.

To undo, connect the watch, click **Restore original font**, then unplug and restart it again.

The app is in Georgian, with an English switch in the top corner.

## Command line

The same installer runs from a terminal; download
`georgian-for-garmin-<version>-macos-universal.tar.gz` from Releases.

```sh
georgian-for-garmin inspect          # watch model, font pack, tested status, staged files
georgian-for-garmin install          # stage Georgian in every size of the watch's font pack
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
