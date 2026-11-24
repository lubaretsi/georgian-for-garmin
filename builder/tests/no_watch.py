"""The one guard for tests that send the real CLI a call that opens a watch (`wait_for_watch`,
`inspect`, `extdata_list`, …). Such a call opens any Garmin watch plugged into the computer, and
any environment variable is only a claim, so these tests check the real condition: the USB
devices are listed without opening any, and the tests run only when that worked and found no
Garmin device (vendor 0x091E), and GFG_TEST_NO_WATCH=1 opts in. Linux reads sysfs; macOS reads
the IOKit registry (`ioreg -p IOUSB`); other systems aren't covered, so the tests skip there."""

import os
import re
import subprocess
import sys
from pathlib import Path

import pytest

GARMIN_VENDOR_ID = 0x091E
SYSFS = Path("/sys")


class NotListed(Exception):
    """The USB devices couldn't be listed."""


def garmin_devices(platform=sys.platform, sysfs=SYSFS, run=subprocess.run):
    """How many Garmin USB devices are attached, read without opening any. Raises NotListed
    when this system's USB devices can't be listed."""
    if platform.startswith("linux"):
        if not (sysfs / "bus").is_dir():
            raise NotListed(f"{sysfs / 'bus'} is missing: sysfs isn't mounted")
        devices = sysfs / "bus/usb/devices"
        # No USB bus at all (no `usb` under sysfs' buses): no Garmin device either.
        vendors = devices.glob("*/idVendor") if (sysfs / "bus/usb").exists() else []
        try:
            return sum(int(v.read_text().strip(), 16) == GARMIN_VENDOR_ID for v in vendors)
        except (OSError, ValueError) as e:
            raise NotListed(f"can't read {devices}: {e}") from e
    if platform == "darwin":
        try:
            out = run(["/usr/sbin/ioreg", "-p", "IOUSB", "-l", "-w0"], capture_output=True,
                      text=True, check=True, timeout=30).stdout
        except (OSError, subprocess.SubprocessError) as e:
            raise NotListed(f"ioreg failed: {e}") from e
        return sum(int(v) == GARMIN_VENDOR_ID for v in re.findall(r'"idVendor" = (\d+)', out))
    raise NotListed(f"listing USB devices without opening them isn't covered on {platform}")


def skip_reason(environ=os.environ, count=garmin_devices):
    """Why a test that opens a watch must not run here, or None when it may."""
    if environ.get("GFG_TEST_NO_WATCH") != "1":
        return "opens any Garmin watch attached over USB: opt in with GFG_TEST_NO_WATCH=1"
    if not environ.get("GEORGIAN_FOR_GARMIN_CLI"):
        return "set GEORGIAN_FOR_GARMIN_CLI to a dev build of the CLI"
    try:
        found = count()
    except NotListed as e:
        return f"opens any Garmin watch attached, and USB couldn't be checked for one ({e})"
    if found:
        return f"{found} Garmin USB device(s) attached, which this test would open"
    return None


def opens_a_watch(test, environ=os.environ, count=garmin_devices):
    """Skips `test` unless skip_reason finds nothing against running it."""
    reason = skip_reason(environ, count)
    return pytest.mark.skipif(reason is not None, reason=reason or "no Garmin device attached")(test)
