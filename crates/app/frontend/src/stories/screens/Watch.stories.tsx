import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import type { Verdict, WatchInfo } from "../../model/screens";
import { issueOf, unsupportedReason, WATCHES } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

const watch = (w: WatchInfo, verdict: Verdict, extra: { issue?: number | null; reportWanted?: boolean; sharedBefore?: boolean } = {}) =>
  ({
    screen: "watch",
    watch: w,
    verdict,
    issue: extra.issue ?? null,
    reportWanted: extra.reportWanted ?? false,
    sharedBefore: extra.sharedBefore ?? false,
  }) as const;

export default {
  title: "Screens/Watch",
  render: renderScreen,
  args: { onAction: fn(), model: watch(WATCHES.venu2, { kind: "install", confidence: "confirmed" }) },
} satisfies Meta<ScreenStoryArgs>;

// install: every confidence
export const InstallConfirmed: ScreenStoryObj = {
  args: { model: watch(WATCHES.venu2, { kind: "install", confidence: "confirmed" }), restorable: true },
};
export const InstallConfirmedOtherSoftware: ScreenStoryObj = {
  args: { model: watch(WATCHES.forerunner255Newer, { kind: "install", confidence: "confirmed_other_software" }) },
};
export const InstallConfirmedSomeFonts: ScreenStoryObj = {
  args: {
    model: watch(WATCHES.instinct3, { kind: "install", confidence: "confirmed_some_fonts" }, { issue: 6, reportWanted: true }),
  },
};
export const InstallUnconfirmed: ScreenStoryObj = {
  args: { model: watch(WATCHES.lily2Active, { kind: "install", confidence: "unconfirmed" }, { reportWanted: true }) },
};

// install: every note
export const InstallNotePartly: ScreenStoryObj = {
  args: { model: watch(WATCHES.venu2, { kind: "install", confidence: "confirmed", note: "partly" }), restorable: true },
};
export const InstallNoteBeta: ScreenStoryObj = {
  args: {
    model: watch(WATCHES.instinct3, { kind: "install", confidence: "unconfirmed", note: "beta" }, { reportWanted: true }),
    restorable: true,
  },
};
export const InstallNoteNotVisible: ScreenStoryObj = {
  args: {
    model: watch(WATCHES.forerunner165, { kind: "install", confidence: "confirmed", note: "not_visible" }, { issue: issueOf("errors", "inspect-restore_first") }),
    restorable: true,
  },
};

// installed: every confidence
export const InstalledConfirmed: ScreenStoryObj = {
  args: { model: watch(WATCHES.venu2, { kind: "installed", confidence: "confirmed" }), restorable: true },
};
export const InstalledConfirmedOtherSoftware: ScreenStoryObj = {
  args: {
    model: watch(WATCHES.forerunner255Newer, { kind: "installed", confidence: "confirmed_other_software" }),
    restorable: true,
  },
};
export const InstalledConfirmedSomeFonts: ScreenStoryObj = {
  args: { model: watch(WATCHES.instinct3, { kind: "installed", confidence: "confirmed_some_fonts" }), restorable: true },
};
export const InstalledUnconfirmed: ScreenStoryObj = {
  args: { model: watch(WATCHES.lily2Active, { kind: "installed", confidence: "unconfirmed" }), restorable: true },
};

export const RestartPendingGeorgian: ScreenStoryObj = {
  args: { model: watch(WATCHES.venu2, { kind: "restart_pending", variant: "georgian" }), restorable: true },
};
export const RestartPendingStock: ScreenStoryObj = {
  args: { model: watch(WATCHES.venu2, { kind: "restart_pending", variant: "stock" }) },
};

export const BlockedWaitingOtherFont: ScreenStoryObj = {
  args: { model: watch(WATCHES.venu3, { kind: "blocked", reason: "waiting_other_font" }, { issue: 5, reportWanted: true }) },
};
export const BlockedUnknownFont: ScreenStoryObj = {
  args: { model: watch(WATCHES.venu2, { kind: "blocked", reason: "unknown_font" }, { reportWanted: true }) },
};

/** fenix 6S Pro (issue 6): no earlier build seen, so Restore stays in More only (§5.1 rule 1). */
export const UnsupportedFollowUpdates: ScreenStoryObj = {
  args: {
    model: watch(
      WATCHES.fenix6sPro,
      { kind: "unsupported", reason: unsupportedReason(), restorable: false },
      { issue: issueOf("fenix6"), reportWanted: true },
    ),
    restorable: true,
  },
};
export const UnsupportedRestorable: ScreenStoryObj = {
  args: {
    model: watch(
      WATCHES.fenix6sPro,
      { kind: "unsupported", reason: unsupportedReason(), restorable: true },
      { issue: issueOf("fenix6-beta-left") },
    ),
    restorable: true,
  },
};

export const NoFont: ScreenStoryObj = {
  args: { model: watch(WATCHES.forerunner265, { kind: "no_font" }, { reportWanted: true }) },
};
export const NoFontSharedBefore: ScreenStoryObj = {
  args: { model: watch(WATCHES.forerunner265, { kind: "no_font" }, { reportWanted: true, sharedBefore: true }) },
};

export const Unreadable: ScreenStoryObj = {
  args: { model: watch(WATCHES.venu2, { kind: "unreadable" }) },
};
