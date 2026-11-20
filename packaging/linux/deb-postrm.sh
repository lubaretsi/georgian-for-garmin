#!/bin/sh
# postrm of the app's .deb: stop udev using the rule the package removed.
set -e
if { [ "$1" = remove ] || [ "$1" = purge ]; } && [ -d /run/udev ] && command -v udevadm >/dev/null 2>&1; then
  udevadm control --reload || true
fi
