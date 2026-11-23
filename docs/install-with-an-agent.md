# Install Georgian with an AI agent

Instructions for an AI agent installing Georgian on a user's Garmin watch without the Georgian
for Garmin app. The procedure copies font files to the watch over USB: every part of the
manifest's packs that the watch lists, across all packs. The watch imports them after it's
unplugged and restarted. It doesn't modify firmware. [How it works](how-it-works.md) explains the
background. If the user can run this project's [command-line tool](command-line.md),
`georgian-for-garmin install` does all of the steps below.

**Verified:** Venu 2, firmware 19.05, from macOS. With all four Armenian sizes installed
(21, 24, 27 and 31 px), Georgian renders in the notification pop-up, the notification list and an
opened notification. Restoring Garmin's original 27 px font alone brings the boxes back in the
pop-up. Other watches may work if their `GarminDevice.xml` lists a part from the manifest (step 3).
From Windows 11 the same files were copied and read back through Windows' built-in driver; the
watch wasn't checked after a Windows-only install.

## Rules

- Ask the user before writing to the watch.
- Write only the `.tmp` files of the parts you picked in step 3, each under the `FileName` its
  `UpdateFile` entry declares (`006-D9035-03.tmp`, or a short form such as `D894567.tmp`).
  Don't delete or change other files, create folders, or touch firmware files.
- Check every target before writing any. A part's waiting file may be under its part number or
  Garmin's short form (`006-D9035-03.tmp` or `D903503.tmp`, any case). If one is identical to
  the file you would write and has the declared name, skip that part; if one holds different
  contents or another name, stop without writing anything and tell the user (it's waiting to be
  imported; unplugging and restarting the watch clears it).
- Read each file back and compare SHA-256 before moving to the next.
- If a part fails, delete every file this run created, tell the user which deletions worked, and
  stop. Leave files staged by earlier runs alone.

## Steps

1. **Get the fonts.** Use `manifest.json`, `georgian/` and `stock/` from a release, or build them
   with `nix build github:lubaretsi/georgian-for-garmin#fonts` (output in `result/`). `packs`
   lists each pack in order with its `kind`, its `parts` and the watches it was `tested` on;
   `fonts` gives each part's `pack`, file paths and SHA-256s, and a bitmap part's `size_px`.
   This procedure covers `bitmap` packs only. `truetype` packs replace a font the watch already
   uses (`<part>.bin`), which needs the original saved and written back on any failure; install
   those only with the app or the command-line tool.
2. **Connect.** Have the user quit Garmin Express (it holds the USB connection) and connect the
   watch by cable. It appears as an MTP device with USB vendor ID `0x091E`. On macOS and Linux,
   use [libmtp](https://github.com/libmtp/libmtp)'s C API (from Python via `ctypes`, for example).
   On Windows, use the built-in Windows Portable Devices API: an object's ID is `o` plus its MTP
   handle in hex without leading zeros (`o1000009`), which also reaches unlisted objects. Create
   files with `CreateObjectWithPropertiesAndData` (parent = the `ExtData` ID), read them back
   through their default resource, and delete with `Delete`. Don't install another driver.
3. **Pick the targets.** Read `GARMIN/GarminDevice.xml`. Take the `MassStorageMode/UpdateFile`
   entries whose `Path` is `Garmin/ExtData` (compare case-insensitively). The targets are every
   part of every `bitmap` pack in the manifest's `packs` that is among their `PartNumber`s,
   across those packs, each staged under its entry's `FileName`. If none matches, stop: tell
   the user the watch isn't supported yet and offer to share its report (below). Tell the user
   the model, software version
   (`Model/Description`, `Model/SoftwareVersion`; `1905` is `19.05`), the targets, and whether
   it's tested: a part is tested when its pack's `tested` list has an entry with this
   `Model/PartNumber` and software version that names the part, and the watch is tested only
   when every target is.
4. **Find `ExtData`.** When it's empty, the watch omits it from folder listings, and creating it
   fails. Try in order and stop at the first match:
   1. children of the `GARMIN` folder;
   2. all objects in its storage (`GetObjectHandles` with parent `0`): an object named `ExtData`, or
      the parent of an existing `006-D*.tmp` file;
   3. `GetObjectInfo` (`LIBMTP_Get_Filemetadata`) on the 32 handles after
      `GARMIN handle & 0xFF000000`.

   Accept a handle only if it's named `ExtData`, is a folder (association `0x3001`), its parent is
   `GARMIN`, and it's on the same storage. On the verified Venu 2 it's `0x01000009`. If none
   matches, stop.
5. **Check the targets** against what `ExtData` holds, as in the rules.
6. **Copy and verify each part.** Send `georgian/<part>.fnt` into `ExtData` under the part's
   declared `FileName` (`SendObjectInfo` + `SendObject`). With libmtp, call
   `LIBMTP_Send_File_From_File` with
   `parent_id` set to the `ExtData` handle and the matching `storage_id`. libmtp's command-line
   tools resolve folders by listing, so they can't reach `ExtData`. Read the file back
   (`LIBMTP_Get_File_To_File`) and compare SHA-256 with the manifest.
7. **Import.** Have the user unplug the watch, wait for it to settle, then power it off and on.
   The `.tmp` files disappear when the watch takes them.
8. **Check.** Have the user send a phone notification containing `მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ`. Both words
   should show letters, not boxes.

## Undo

Repeat steps 4–7 with Garmin's originals, `stock/<part>.fnt` from the same manifest. Verified only
on the Venu 2 at firmware 19.05 for the 27 px part; other sizes and models haven't been restored on
a watch, so tell the user that.

## Untested and unsupported watches

If any target isn't tested on this model and software version, or no part matches,
ask the user to share the watch's font list in a
[watch report issue](https://github.com/lubaretsi/georgian-for-garmin/issues/new?template=watch-report.yml)
(the form's **Watch report** field):
model, `Model/PartNumber`, software version, and each `UpdateFile`'s path, file name, part number
and version (`Major`.`Minor`, two-digit minor: `2.10`). Never include the unit ID or serial number
(the `Id` elements). `georgian-for-garmin inspect --report` prints this report, and
`georgian-for-garmin issue-link` makes a link with the form filled in
([Reporting](reporting.md)).
