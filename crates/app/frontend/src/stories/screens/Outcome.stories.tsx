import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import type { WatchInfo } from "../../model/screens";
import { WATCHES } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

type Kind = "works_confirmed" | "works_first" | "didnt_show" | "no_notification";
const outcome = (kind: Kind, watch: WatchInfo, restarts = 0, sharedBefore = false) =>
  ({ screen: "outcome", watch, kind, restarts, sharedBefore }) as const;

export default {
  title: "Screens/Outcome",
  render: renderScreen,
  args: { onAction: fn(), model: outcome("works_confirmed", WATCHES.venu2) },
} satisfies Meta<ScreenStoryArgs>;

export const WorksConfirmed: ScreenStoryObj = { args: { model: outcome("works_confirmed", WATCHES.venu2, 1), restorable: true } };

/** Lily 2 Active: the first confirmation, from the owner's email. */
export const WorksFirst: ScreenStoryObj = { args: { model: outcome("works_first", WATCHES.lily2Active, 1), restorable: true } };
export const WorksFirstSharedBefore: ScreenStoryObj = {
  args: { model: outcome("works_first", WATCHES.lily2Active, 1, true), restorable: true },
};

/** Instinct 3 (issue 33): boxes. Before any restart the app suggests one. */
export const DidntShowRestarts0: ScreenStoryObj = {
  args: { model: outcome("didnt_show", WATCHES.instinct3, 0), restorable: true },
};
/** After two restarts sharing comes first. */
export const DidntShowRestarts2: ScreenStoryObj = {
  args: { model: outcome("didnt_show", WATCHES.instinct3, 2), restorable: true },
};
export const DidntShowSharedBefore: ScreenStoryObj = {
  args: { model: outcome("didnt_show", WATCHES.instinct3, 2, true), restorable: true },
};

export const NoNotification: ScreenStoryObj = { args: { model: outcome("no_notification", WATCHES.lily2Active, 1) } };
