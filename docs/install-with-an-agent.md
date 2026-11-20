# Install Georgian with an AI agent

Instructions for an AI agent installing Georgian on a user's Garmin watch without the Georgian
for Garmin app. The procedure copies one font file to the watch over USB. The watch imports it after
it's unplugged and restarted. It doesn't modify firmware.

**Verified:** Venu 2, firmware 19.05, from macOS with libmtp. Other watches may work if their
`GarminDevice.xml` lists the same font part (step 3). Windows hasn't been verified.

## Rules

- Ask the user before writing to the watch.
- Write only `GARMIN/ExtData/006-D9035-03.tmp`. Don't delete or change other files, create folders,
  or touch firmware files.
- If `ExtData` already holds a `006-D9035-03.tmp` with different contents, stop and tell the user.
- Read the file back and compare SHA-256 before telling the user to unplug.

## Steps

1. **Get the font.** Use `georgian/006-D9035-03.fnt` and its SHA-256 from `manifest.json` in a
   release, or build them with `nix build github:lubaretsi/georgian-for-garmin#fonts` (output in
   `result/`). It's Garmin's 27 px Armenian font with 66 Georgian letters added.
2. **Connect.** Have the user quit Garmin Express (it holds the USB connection) and connect the
   watch by cable. It appears as an MTP device with USB vendor ID `0x091E`. On macOS and Linux,
   use [libmtp](https://github.com/libmtp/libmtp)'s C API (from Python via `ctypes`, for example).
3. **Check support.** Read `GARMIN/GarminDevice.xml`. Continue only if a
   `MassStorageMode/UpdateFile` has `PartNumber` `006-D9035-03`, `Path` `Garmin/ExtData`, and
   `FileName` `006-D9035-03.tmp`. Tell the user the model and software version (`Model/Description`,
   `Model/SoftwareVersion`).
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
5. **Copy.** Send `006-D9035-03.tmp` into `ExtData` (`SendObjectInfo` + `SendObject`). With
   libmtp, call `LIBMTP_Send_File_From_File` with `parent_id` set to the `ExtData` handle and the
   matching `storage_id`. libmtp's command-line tools resolve folders by listing, so they can't
   reach `ExtData`.
6. **Verify the copy.** Read the file back (`LIBMTP_Get_File_To_File`) and compare SHA-256 with
   the manifest.
7. **Import.** Have the user unplug the watch, wait for it to settle, then power it off and on.
   The `.tmp` file disappears when the watch takes it.
8. **Check.** Have the user send a phone notification containing `ქართული ᲥᲐᲠᲗᲣᲚᲘ`. Both words
   should show letters, not boxes.

## Undo

Repeat steps 4–7 with Garmin's original `stock/006-D9035-03.fnt` from the same manifest. Restoring
hasn't been verified on a watch yet.
