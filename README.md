<img src="crates/app/icons/icon.png" alt="Georgian for Garmin logo" width="96" align="right">

# Georgian for Garmin

ქართული შრიფტი Garmin-ის საათებისთვის.

Garmin watches show Georgian text in notifications as boxes. This project adds Georgian letters to
a font the watch already uses, so notifications from your phone show `მხედრული` and `ᲛᲗᲐᲕᲠᲣᲚᲘ`
instead of `□□□□□□□□`.

<p>
  <a href="https://github.com/lubaretsi/georgian-for-garmin/releases/latest/download/Georgian-for-Garmin-macos.zip"><img src="https://img.shields.io/badge/macOS-Download-000000?style=for-the-badge&logo=apple&logoColor=white" alt="Download for macOS"></a>
  <a href="https://github.com/lubaretsi/georgian-for-garmin/releases/latest/download/Georgian-for-Garmin-windows-x64-setup.exe"><img src="https://img.shields.io/badge/Windows-Download-0078D4?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0id2hpdGUiIGQ9Ik0yIDJoOS41djkuNUgyek0xMi41IDJIMjJ2OS41aC05LjV6TTIgMTIuNWg5LjVWMjJIMnpNMTIuNSAxMi41SDIyVjIyaC05LjV6Ii8%2BPC9zdmc%2B" alt="Download for Windows"></a>
  <a href="https://github.com/lubaretsi/georgian-for-garmin/releases/latest/download/Georgian-for-Garmin-linux-x86_64.deb"><img src="https://img.shields.io/badge/Linux-Download_.deb-333333?style=for-the-badge&logo=linux&logoColor=white" alt="Download for Linux (.deb)"></a>
</p>

The app isn't signed, so your computer warns the first time; [Install](#install) shows what to
click. Windows on Arm, other Linux and the command-line tools:
[all downloads](https://github.com/lubaretsi/georgian-for-garmin/releases/latest).

Installing on Windows, start to finish in 46 seconds
([step by step in Georgian](docs/install-windows.md)):

https://github.com/user-attachments/assets/c4adbad8-d956-4ab3-ba71-5f67e415e6df

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
| tactix 7 | 27.18 | Tested: Georgian in notifications |
| Venu 2 Plus | 19.05 | Tested by a user: Georgian in notifications ([#4](https://github.com/lubaretsi/georgian-for-garmin/issues/4)) |
| fenix 7 Pro Solar, Pro Sapphire Solar | 26.09 | Tested by users: Georgian in notifications and incoming calls ([#11](https://github.com/lubaretsi/georgian-for-garmin/issues/11), [#15](https://github.com/lubaretsi/georgian-for-garmin/issues/15)) |

The app reads the font list of the connected watch and adds Georgian to every Garmin Armenian
font it uses, in any size: NotoSans Regular, NotoSans Armenian Regular and NotoSans Armenian
Bold. Other models and software versions are untested until someone confirms them. Reporting
takes a minute and only a GitHub account: after installing, click **Check on the watch**, answer
what you saw, and **Share on GitHub** opens the issue form already filled in
([Reporting](docs/reporting.md)).

## Install

You need your watch's USB data cable. Download the app for your computer from
[Releases](https://github.com/lubaretsi/georgian-for-garmin/releases):

| Computer | Download, then |
|---|---|
| Mac, macOS 11 or later | `Georgian-for-Garmin-macos.zip`: unzip it and open **Georgian for Garmin** |
| Windows 10 or 11 | `Georgian-for-Garmin-windows-x64-setup.exe` (`-arm64-` on Arm PCs): run it |
| Debian, Ubuntu | `Georgian-for-Garmin-linux-x86_64.deb` (`-aarch64` on Arm): `sudo apt install ./Georgian-for-Garmin-linux-*.deb` |
| Other Linux | `Georgian-for-Garmin-linux-x86_64.AppImage`: `chmod +x` it, run it, and [install the udev rule](docs/command-line.md#linux) |

The app isn't signed, so the first time:

- **macOS** blocks it: open **System Settings → Privacy & Security** and click **Open Anyway**.
- **Windows** says "Windows protected your PC": click **More info → Run anyway**.

Step by step on Windows, in Georgian, with every screen: [Windows-ზე დაყენება](docs/install-windows.md).

Then:

1. Quit Garmin Express if it's running, and connect the watch. If the watch asks how to connect,
   choose MTP. On Linux, [release it from GNOME or KDE](docs/command-line.md#linux) first.
2. Click **Install Georgian** (**ქართულის დაყენება**).
3. Unplug the watch and restart it: hold the top button, choose **Power Off**, then turn it on.
   If you still see boxes, restart the watch once more.
4. Check it: click **Check on the watch** (**საათზე შემოწმება**), scan its code with your phone
   and tap **Send test notification**, or send yourself `Test მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ Test`.

To undo, connect the watch, click **Restore original font**, then unplug and restart it again.

The app is in Georgian, with an English switch in the top corner. The macOS app is tested with a
watch. On Windows (11 on Arm) and Linux (Ubuntu 24.04) the command-line installer is tested with a
watch; the Windows and Linux apps are tested only without one so far. The Linux app needs glibc
2.34 or newer.

## Command line

The same installer runs from a terminal. Download `georgian-for-garmin-` plus
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
