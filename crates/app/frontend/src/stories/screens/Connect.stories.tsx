import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import type { BusyView, ConnectProblem } from "../../model/screens";
import { busyFrom, messageOf } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

export default {
  title: "Screens/Connect",
  render: renderScreen,
  args: { onAction: fn(), model: { screen: "connect", mode: "searching", showHelp: false } },
} satisfies Meta<ScreenStoryArgs>;

export const Searching: ScreenStoryObj = {
  args: { model: { screen: "connect", mode: "searching", showHelp: false } },
};

export const SearchingWithHelp: ScreenStoryObj = {
  args: { model: { screen: "connect", mode: "searching", showHelp: true } },
};

/** The watch came up in Garmin mode, and the app switches it to MTP (§4.2). */
export const Switching: ScreenStoryObj = {
  args: { model: { screen: "connect", mode: "switching" } },
};

// Busy: each holder kind × platform. Holders come from the core's busy fixtures; the
// combinations no fixture has reuse the same kind's holder as that platform reports it.
const busy = (b: BusyView, duringRestart = false) => ({ screen: "connect", mode: "busy", busy: b, duringRestart }) as const;
const express = busyFrom("inspect-garmin_express-macos");
const expressWindows = busyFrom("inspect-garmin_express-windows");
const camera = busyFrom("inspect-macos_camera_service-macos");
const mtpMac = busyFrom("inspect-desktop_mtp-macos");
const mtpLinux = busyFrom("inspect-desktop_mtp-linux");
const otherLinux = busyFrom("inspect-other-linux");
const otherCut = busyFrom("inspect-other-macos-cut");
const unknown = busyFrom("inspect-unknown-linux");
const thisApp = busyFrom("inspect-this_app-macos");
const thisAppWindows = busyFrom("inspect-this_app-windows");

export const BusyGarminExpressMacos: ScreenStoryObj = { args: { platform: "macos", model: busy(express) } };
export const BusyGarminExpressWindows: ScreenStoryObj = { args: { platform: "windows", model: busy(expressWindows) } };
export const BusyGarminExpressLinux: ScreenStoryObj = { args: { platform: "linux", model: busy(express) } };
export const BusyCameraServiceMacos: ScreenStoryObj = { args: { platform: "macos", model: busy(camera) } };
export const BusyCameraServiceWindows: ScreenStoryObj = {
  args: { platform: "windows", model: busy({ ...camera, confirmed: false }) },
};
export const BusyCameraServiceLinux: ScreenStoryObj = { args: { platform: "linux", model: busy(camera) } };
export const BusyDesktopMtpMacos: ScreenStoryObj = { args: { platform: "macos", model: busy(mtpMac) } };
export const BusyDesktopMtpWindows: ScreenStoryObj = {
  args: { platform: "windows", model: busy({ ...mtpMac, confirmed: false }) },
};
export const BusyDesktopMtpLinux: ScreenStoryObj = { args: { platform: "linux", model: busy(mtpLinux) } };
export const BusyOtherMacosNameCut: ScreenStoryObj = { args: { platform: "macos", model: busy(otherCut) } };
export const BusyOtherWindows: ScreenStoryObj = {
  args: { platform: "windows", model: busy({ ...otherLinux, confirmed: false }) },
};
export const BusyOtherLinux: ScreenStoryObj = { args: { platform: "linux", model: busy(otherLinux) } };
export const BusyUnknownMacos: ScreenStoryObj = { args: { platform: "macos", model: busy(unknown) } };
export const BusyUnknownWindows: ScreenStoryObj = { args: { platform: "windows", model: busy(unknown) } };
export const BusyUnknownLinux: ScreenStoryObj = { args: { platform: "linux", model: busy(unknown) } };
/** This very app still has the watch open for an earlier step. */
export const BusyThisAppMacos: ScreenStoryObj = { args: { platform: "macos", model: busy(thisApp) } };
export const BusyThisAppWindows: ScreenStoryObj = { args: { platform: "windows", model: busy(thisAppWindows) } };
export const BusyThisAppLinux: ScreenStoryObj = { args: { platform: "linux", model: busy(thisApp) } };
/** Garmin Express takes the watch while the app waits for it after a restart. */
export const BusyDuringRestart: ScreenStoryObj = { args: { platform: "macos", model: busy(express, true) } };

const problem = (p: ConnectProblem, detail: string) => ({ screen: "connect", mode: "problem", problem: p, detail }) as const;

export const ProblemPermissionDenied: ScreenStoryObj = {
  args: { platform: "linux", model: problem("permission_denied", messageOf("connect-problems", "inspect-permission_denied")) },
};
export const ProblemMultipleDevices: ScreenStoryObj = {
  args: { model: problem("multiple_devices", messageOf("connect-problems", "inspect-multiple_devices")) },
};
export const ProblemGarminFolderNotFound: ScreenStoryObj = {
  args: { model: problem("garmin_folder_not_found", messageOf("connect-problems", "inspect-garmin_folder_not_found")) },
};
export const ProblemExtdataNotFound: ScreenStoryObj = {
  args: { model: problem("extdata_not_found", messageOf("connect-problems", "install-extdata_not_found")) },
};
export const ProblemDeviceError: ScreenStoryObj = {
  args: { model: problem("device_error", messageOf("connect-problems", "inspect-device_error")) },
};
export const ProblemInternal: ScreenStoryObj = {
  args: { model: problem("internal", messageOf("connect-problems", "inspect-internal")) },
};
/** Windows can't switch a watch in Garmin mode: what to do on the watch, straight away. */
export const ProblemGarminModeWindows: ScreenStoryObj = {
  args: { platform: "windows", model: problem("garmin_mode", messageOf("garmin-mode", "inspect-garmin_mode-windows")) },
};
/** The switch didn't bring the watch back: the same guidance. */
export const ProblemGarminModeSwitchFailed: ScreenStoryObj = {
  args: { model: problem("garmin_mode", messageOf("garmin-mode", "switch_to_mtp-unsupported_device")) },
};
export const ProblemDeviceLocked: ScreenStoryObj = {
  args: { model: problem("device_locked", messageOf("garmin-mode", "switch_to_mtp-device_locked")) },
};
