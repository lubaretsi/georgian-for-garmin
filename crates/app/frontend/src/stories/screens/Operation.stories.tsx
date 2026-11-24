import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import { output } from "../../mock/fixtures";
import type { WatchInfo } from "../../model/screens";
import { WATCHES } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

export default {
  title: "Screens/Install and restart",
  render: renderScreen,
  args: { onAction: fn(), model: { screen: "confirm_restore", watch: WATCHES.venu2 } },
} satisfies Meta<ScreenStoryArgs>;

/** Files the real install and restore wrote on the Venu 2 (fixtures venu2/install, venu2/restore_stock). */
const filesOf = (op: "install" | "restore_stock") =>
  output("venu2", op, op).packs.reduce((n, pack) => n + pack.parts.length, 0);
const venu2Files = filesOf("install");
const restoreFiles = filesOf("restore_stock");

export const ConfirmRestore: ScreenStoryObj = {
  args: { model: { screen: "confirm_restore", watch: WATCHES.venu2 }, restorable: true },
};

export const WorkingInstall: ScreenStoryObj = {
  args: { model: { screen: "working", op: "install", watch: WATCHES.venu2, progress: null } },
};
export const WorkingInstallProgress: ScreenStoryObj = {
  args: { model: { screen: "working", op: "install", watch: WATCHES.venu2, progress: { done: 1, total: venu2Files } } },
};
export const WorkingRestore: ScreenStoryObj = {
  args: { model: { screen: "working", op: "restore", watch: WATCHES.venu2, progress: null } },
};
export const WorkingRestoreProgress: ScreenStoryObj = {
  args: {
    model: { screen: "working", op: "restore", watch: WATCHES.venu2, progress: { done: restoreFiles - 1, total: restoreFiles } },
  },
};

const restart = (
  phase: "unplug" | "restart" | "verifying" | "again" | "other_watch",
  w: WatchInfo = WATCHES.venu2,
  extra: { attempt?: number; verifiable?: boolean; op?: "install" | "restore" } = {},
) =>
  ({
    screen: "restart",
    op: extra.op ?? "install",
    watch: w,
    phase,
    attempt: extra.attempt ?? 1,
    verifiable: extra.verifiable ?? true,
  }) as const;

export const RestartUnplug: ScreenStoryObj = { args: { model: restart("unplug") } };
export const RestartUnplugRestore: ScreenStoryObj = { args: { model: restart("unplug", WATCHES.venu2, { op: "restore" }) } };
export const RestartRestart: ScreenStoryObj = { args: { model: restart("restart", WATCHES.lily2Active) } };
export const RestartVerifying: ScreenStoryObj = { args: { model: restart("verifying") } };
export const RestartAgain: ScreenStoryObj = { args: { model: restart("again") } };
export const RestartAttempt2: ScreenStoryObj = { args: { model: restart("restart", WATCHES.venu2, { attempt: 2 }) } };
export const RestartOtherWatch: ScreenStoryObj = { args: { model: restart("other_watch", WATCHES.lily2Active) } };
/** Venu 3 doesn't show its font files, so the app can't see the change. */
export const RestartNotVerifiable: ScreenStoryObj = {
  args: { model: restart("restart", WATCHES.venu3, { verifiable: false }) },
};
export const RestartNotVerifiableRestore: ScreenStoryObj = {
  args: { model: restart("restart", WATCHES.venu3, { verifiable: false, op: "restore" }) },
};

export const ResultInstallConfirmed: ScreenStoryObj = {
  args: { model: { screen: "result", op: "install", watch: WATCHES.venu2, confidence: "confirmed" }, restorable: true },
};
export const ResultInstallUnconfirmed: ScreenStoryObj = {
  args: { model: { screen: "result", op: "install", watch: WATCHES.lily2Active, confidence: "unconfirmed" }, restorable: true },
};
export const ResultRestore: ScreenStoryObj = {
  args: { model: { screen: "result", op: "restore", watch: WATCHES.venu2, confidence: "confirmed" } },
};
