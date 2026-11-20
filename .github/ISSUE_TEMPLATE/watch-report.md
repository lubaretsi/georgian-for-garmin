---
name: Watch report
about: Share your watch's font list so it can be added or marked tested
title: "Watch report: <model> (<model part number>), software <version>"
labels: watch-report
---

<!-- The app's "Share my watch's font list" button fills this in for you, or run
`georgian-for-garmin inspect --report` and paste its output over this text.
Never paste the watch's unit ID or serial number. -->

## Watch

- Model: <!-- Model/Description in GARMIN/GarminDevice.xml, e.g. Venu 2 -->
- Model part number: <!-- Model/PartNumber, e.g. 006-B3703-00 -->
- Software version: <!-- Model/SoftwareVersion with two decimals: 1905 is 19.05 -->
- Georgian for Garmin: <!-- app or CLI version -->
- Font pack: <!-- the pack the app picked and whether it was tested, or "none of this build's packs" -->

## Fonts the watch lists in Garmin/ExtData

<!-- One row per MassStorageMode/UpdateFile whose Path is Garmin/ExtData.
Version is Major.Minor with a two-digit minor: Major 2, Minor 10 is 2.10. -->

| Part number | Version | File name |
| --- | --- | --- |
|  |  |  |

## Result

<!-- If you installed Georgian: after unplugging and restarting the watch, did a notification
with მხედრული ᲛᲗᲐᲕᲠᲣᲚᲘ show letters instead of boxes? -->
