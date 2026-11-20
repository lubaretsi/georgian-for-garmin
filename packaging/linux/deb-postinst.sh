#!/bin/sh
# postinst of the app's .deb: load the udev rule it installed into /usr/lib/udev/rules.d/ and apply
# it to a Garmin device that's already plugged in. Skipped where udev isn't running (containers).
set -e
if [ "$1" = configure ] && [ -d /run/udev ] && command -v udevadm >/dev/null 2>&1; then
  udevadm control --reload || true
  udevadm trigger --subsystem-match=usb --attr-match=idVendor=091e || true
fi
