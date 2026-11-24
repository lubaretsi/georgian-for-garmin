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
export const Lily2ActiveFirstConfirmation: Story = { args: { options: { watch: "lily2active" } } };
export const VenuBoxes: Story = { args: { options: { watch: "venuBoxes" } } };
export const Instinct3UnsupportedRestore: Story = { args: { options: { watch: "instinct3" } } };
export const Venu3AnotherFontWaiting: Story = { args: { options: { watch: "venu3" } } };
export const Fenix6Unsupported: Story = { args: { options: { watch: "fenix6" } } };
export const Fenix6EarlierTestVersionLeft: Story = { args: { options: { watch: "fenix6BetaLeft" } } };
/** Plug it in as a drive: it has no MTP. */
export const Forerunner245DriveOnly: Story = { args: { options: { watch: "forerunner245" } } };
export const Forerunner265NoFont: Story = { args: { options: { watch: "forerunner265" } } };
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
