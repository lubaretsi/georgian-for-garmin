import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { output } from "../../mock/fixtures";
import type { WatchInfo } from "../../model/screens";
import { WATCHES } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

const venu2Drive = output("venu2-drive", "inspect_drive", "inspect_drive");
const fr245 = output("forerunner245-drive", "inspect_drive", "inspect_drive");

/**
 * A drive-only watch the support list doesn't have, so the app wants its font list. No fixture
 * has one: the model is real, the part number and software are made up for the story.
 */
const unlisted: WatchInfo = { model: "Forerunner 45", modelPart: "006-B0000-00", software: "4.20", confirmedSoftware: [] };

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

/** Forerunner 245 (issue 16): drive only and already reported, so only Follow updates. */
export const DriveOnlyFollow: ScreenStoryObj = {
  args: {
    model: {
      screen: "drive",
      watch: WATCHES.forerunner245,
      driveOnly: true,
      issue: fr245.support.issue ?? null,
      reportWanted: fr245.support.report_wanted,
      sharedBefore: false,
    },
  },
};

export const DriveOnlyShareFontList: ScreenStoryObj = {
  name: "Drive only, share font list (unlisted watch, made-up data)",
  args: {
    model: { screen: "drive", watch: unlisted, driveOnly: true, issue: null, reportWanted: true, sharedBefore: false },
  },
};

export const DriveOnlySharedBefore: ScreenStoryObj = {
  name: "Drive only, shared before (unlisted watch, made-up data)",
  args: {
    model: { screen: "drive", watch: unlisted, driveOnly: true, issue: null, reportWanted: true, sharedBefore: true },
  },
};
