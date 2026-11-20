# Install Georgian with an AI agent

Instructions for an AI agent installing Georgian on a user's Garmin watch without the Georgian
for Garmin app. The procedure copies font files to the watch over USB: every size of one font
pack that the watch lists. The watch imports them after it's unplugged and restarted. It doesn't
modify firmware. [How it works](how-it-works.md) explains the background. If the user can run
this project's [command-line tool](command-line.md), `georgian-for-garmin install` does all of
the steps below.

**Verified:** Venu 2, firmware 19.05, from macOS. With all four Armenian sizes installed
(21, 24, 27 and 31 px), Georgian renders in the notification pop-up, the notification list and an
opened notification. Restoring Garmin's original 27 px font alone brings the boxes back in the
pop-up. Other watches may work if their `GarminDevice.xml` lists a pack from the manifest (step 3).
Windows can reach the hidden folder with its built-in driver, but this procedure hasn't been run
there.

## Rules

- Ask the user before writing to the watch.
- Write only `GARMIN/ExtData/<part>.tmp` for the parts you picked in step 3. Don't delete or
  change other files, create folders, or touch firmware files.
- Check every target before writing any: if `ExtData` already holds a `<part>.tmp` identical to
  the file you would write, skip that part; if one holds different contents, stop without writing
  anything and tell the user (it's waiting to be imported; unplugging and restarting the watch
  clears it).
- Read each file back and compare SHA-256 before moving to the next.
- If a part fails, delete every file this run created, tell the user which deletions worked, and
  stop. Leave files staged by earlier runs alone.

## Steps

1. **Get the fonts.** Use `manifest.json`, `georgian/` and `stock/` from a release, or build them
   with `nix build github:lubaretsi/georgian-for-garmin#fonts` (output in `result/`). `packs`
   lists each pack in order with its `parts` and the watches it was `tested` on; `fonts` gives
   each part's `pack`, `size_px`, file paths and SHA-256s.
2. **Connect.** Have the user quit Garmin Express (it holds the USB connection) and connect the
   watch by cable. It appears as an MTP device with USB vendor ID `0x091E`. On macOS and Linux,
   use [libmtp](https://github.com/libmtp/libmtp)'s C API (from Python via `ctypes`, for example).
3. **Pick the pack.** Read `GARMIN/GarminDevice.xml`. Take the `MassStorageMode/UpdateFile`
   entries whose `Path` is `Garmin/ExtData` (compare case-insensitively). Pick the first pack in
   the manifest's `packs` with a part among their `PartNumber`s; the targets are that pack's
   parts the watch lists, each staged as `<part>.tmp`. If no pack matches, stop: tell the user the
   watch isn't supported yet and offer to share its report (below). Tell the user the model,
   software version (`Model/Description`, `Model/SoftwareVersion`; `1905` is `19.05`) and
   whether the pack's `tested` list has this `Model/PartNumber` and software version.
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
6. **Copy and verify each part.** Send `georgian/<part>.fnt` as `<part>.tmp` into `ExtData`
   (`SendObjectInfo` + `SendObject`). With libmtp, call `LIBMTP_Send_File_From_File` with
   `parent_id` set to the `ExtData` handle and the matching `storage_id`. libmtp's command-line
   tools resolve folders by listing, so they can't reach `ExtData`. Read the file back
   (`LIBMTP_Get_File_To_File`) and compare SHA-256 with the manifest.
7. **Import.** Have the user unplug the watch, wait for it to settle, then power it off and on.
   The `.tmp` files disappear when the watch takes them.
8. **Check.** Have the user send a phone notification containing `ქართული ᲥᲐᲠᲗᲣᲚᲘ`. Both words
   should show letters, not boxes.

## Undo

Repeat steps 4–7 with Garmin's originals, `stock/<part>.fnt` from the same manifest. Verified on
the Venu 2 at firmware 19.05 for the 27 px part; the other sizes restore the same way.

## Untested and unsupported watches

If the pack's `tested` list doesn't have this model and software version, or no pack matches,
ask the user to share the watch's font list in a
[watch report issue](https://github.com/lubaretsi/georgian-for-garmin/issues/new?template=watch-report.md):
model, `Model/PartNumber`, software version, and each `Garmin/ExtData` `UpdateFile`'s part
number, version (`Major`.`Minor`, two-digit minor: `2.10`) and file name. Never include the
unit ID or serial number (the `Id` elements). `georgian-for-garmin inspect --report` prints
this report.
