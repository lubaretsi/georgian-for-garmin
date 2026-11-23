import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { output } from "../../mock/fixtures";
import { WATCHES } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

const venu2Drive = output("venu2-drive", "inspect_drive", "inspect_drive");
const fr245 = output("forerunner245-drive", "inspect_drive", "inspect_drive");

export default {
  title: "Screens/Drive",
  render: renderScreen,
  args: {
    onAction: fn(),
    model: { screen: "drive", watch: WATCHES.venu2, driveOnly: false, issue: null, reportWanted: false, sharedBefore: false },
  },
} satisfies Meta<ScreenStoryArgs>;

/** Venu 2 connected in drive mode: it can switch to MTP. */
export const SwitchToMtp: ScreenStoryObj = {
  args: {
    model: {
      screen: "drive",
      watch: WATCHES.venu2,
      driveOnly: false,
      issue: venu2Drive.support.issue ?? null,
      reportWanted: venu2Drive.support.report_wanted,
      sharedBefore: false,
    },
  },
};

/** Forerunner 245 (issue 16): drive only. */
export const DriveOnly: ScreenStoryObj = {
  args: {
    model: {
      screen: "drive",
      watch: WATCHES.forerunner245,
      driveOnly: true,
      issue: fr245.support.issue ?? null,
      reportWanted: true,
      sharedBefore: false,
    },
  },
};

export const DriveOnlySharedBefore: ScreenStoryObj = {
  args: {
    model: {
      screen: "drive",
      watch: WATCHES.forerunner245,
      driveOnly: true,
      issue: fr245.support.issue ?? null,
      reportWanted: true,
      sharedBefore: true,
    },
  },
};
