import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { output } from "../../mock/fixtures";
import type { WatchInfo } from "../../model/screens";
import { WATCHES } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

const venu2Drive = output("venu2-drive", "inspect_drive", "inspect_drive");
const driveOnly = output("drive-only", "inspect_drive", "inspect_drive");

/**
 * A drive-only watch the support list doesn't have, so the app wants its font list. No fixture
 * has one: the model, part number and software are made up for the story.
 */
const unlisted: WatchInfo = { model: "Example Unlisted", modelPart: "006-B9999-00", software: "4.20", confirmedSoftware: [] };

/** Venu 2 connected in drive mode: it can switch to MTP. */
const venu2AsDrive = {
  screen: "drive",
  watch: WATCHES.venu2,
  driveOnly: false,
  issue: venu2Drive.support.issue ?? null,
  reportWanted: venu2Drive.support.report_wanted,
  sharedBefore: false,
  duringRestart: false,
} as const;

export default {
  title: "Screens/Drive",
  render: renderScreen,
  args: { onAction: fn(), model: venu2AsDrive },
} satisfies Meta<ScreenStoryArgs>;

export const SwitchToMtp: ScreenStoryObj = { args: { model: venu2AsDrive } };

/** The Venu 2 came back as a USB drive after the restart: the journey stays on Restart. */
export const SwitchToMtpDuringRestart: ScreenStoryObj = { args: { model: { ...venu2AsDrive, duringRestart: true } } };

/** Example Drive Only (issue 16): drive only and already reported, so only Follow updates. */
export const DriveOnlyFollow: ScreenStoryObj = {
  args: {
    model: {
      screen: "drive",
      watch: WATCHES.driveOnly,
      driveOnly: true,
      issue: driveOnly.support.issue ?? null,
      reportWanted: driveOnly.support.report_wanted,
      sharedBefore: false,
      duringRestart: false,
    },
  },
};

const unlistedDriveOnly = {
  screen: "drive",
  watch: unlisted,
  driveOnly: true,
  issue: null,
  reportWanted: true,
  sharedBefore: false,
  duringRestart: false,
} as const;

export const DriveOnlyShareFontList: ScreenStoryObj = {
  name: "Drive only, share font list (unlisted watch, made-up data)",
  args: { model: unlistedDriveOnly },
};

export const DriveOnlySharedBefore: ScreenStoryObj = {
  name: "Drive only, shared before (unlisted watch, made-up data)",
  args: { model: { ...unlistedDriveOnly, sharedBefore: true } },
};
