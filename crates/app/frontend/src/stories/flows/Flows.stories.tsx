// The flows of contract §8 to click through by hand: the whole app over the simulated watch,
// each starting at the beginning with the watch unplugged. Drive the watch with the simulator
// beside the window. The same flows walked end to end by play functions are FlowTests.stories.tsx.

import type { Meta, StoryObj } from "@storybook/react-vite";

import type { MockOptions } from "../../mock/MockBackend";
import { FlowStory } from "./FlowStory";

type Args = { options: MockOptions };

export default {
  title: "Flows",
  render: (args, { globals }) => <FlowStory options={{ ...args.options, connection: "unplugged" }} globals={globals} />,
  parameters: { frame: false, layout: "padded" },
} satisfies Meta<Args>;

type Story = StoryObj<Args>;

export const Venu2Confirmed: Story = { args: { options: { watch: "venu2" } } };
export const UnconfirmedFirstConfirmation: Story = { args: { options: { watch: "unconfirmed" } } };
export const KoreanBoxes: Story = { args: { options: { watch: "koreanBoxes" } } };
export const KoreanUnusedRestore: Story = { args: { options: { watch: "koreanUnused" } } };
export const ExpectedAnotherFontWaiting: Story = { args: { options: { watch: "expectedWaiting" } } };
export const FontUnusedUnsupported: Story = { args: { options: { watch: "fontUnused" } } };
export const FontUnusedEarlierTestVersionLeft: Story = { args: { options: { watch: "fontUnusedBetaLeft" } } };
/** Plug it in as a drive: it has no MTP. */
export const DriveOnly: Story = { args: { options: { watch: "driveOnly" } } };
export const PredictedNoFont: Story = { args: { options: { watch: "predicted" } } };
/** Garmin Express holds the watch from the start: plug it in, then Release. */
export const GarminExpressBusy: Story = { args: { options: { watch: "venu2", holder: "garmin_express" } } };
/** Press Make the next write fail before Install. */
export const InstallFailsRollbackIncomplete: Story = { args: { options: { watch: "venu2" } } };
export const LinuxPermissionDenied: Story = {
  args: { options: { watch: "venu2", platform: "linux", problem: "connect-problems/inspect-permission_denied" } },
  globals: { platform: "linux" },
};
/** Press Next restart doesn't import before the first restart. */
export const SecondRestartNeeded: Story = { args: { options: { watch: "venu2" } } };
/** After the restart, press Garmin Express holds it before plugging in. */
export const GarminExpressGrabsItAfterTheRestart: Story = { args: { options: { watch: "venu2" } } };
/** After the restart, plug in as a drive, then over MTP. */
export const BackAsADriveAfterTheRestart: Story = { args: { options: { watch: "venu2" } } };
/** Garmin Express holds the watch: plug it in, then More › Diagnostic info. */
export const DiagnosticInfoWatchNotFound: Story = { args: { options: { watch: "venu2", holder: "garmin_express" } } };
/** Plug in (Garmin mode): the app switches it. After the restart, plug in (Garmin mode) again. */
export const GarminModeSwitched: Story = { args: { options: { watch: "venu2" } } };
/** Plug in (Garmin mode): no switch on Windows; then Choose MTP on the watch. */
export const GarminModeOnWindows: Story = {
  args: { options: { watch: "venu2", platform: "windows" } },
  globals: { platform: "windows" },
};
/** Plug in (Garmin mode): the watch says it is locked; set the switch to Switches, unplug, plug in again. */
export const GarminModeWatchLocked: Story = { args: { options: { watch: "venu2", switching: "locked" } } };
