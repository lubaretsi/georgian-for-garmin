import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import type { Lang } from "../../core/backend";
import type { ScreenModel, WatchInfo } from "../../model/screens";
import { reportText, shareSummary, WATCHES } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

type What = "result" | "font_list" | "problem";
type State = "ready" | "opened" | "opened_paste" | "failed";

/** Who shares what: the Venu's boxes, Forerunner 265's font list, a problem with no watch. */
const SOURCES: Record<What, { watch: WatchInfo | null; report: [string, string]; thanks: "tested_next" | "we_look" }> = {
  result: { watch: WATCHES.venu, report: ["venu-boxes", "issue_link-boxes"], thanks: "we_look" },
  font_list: { watch: WATCHES.forerunner265, report: ["forerunner265", "issue_link-font_list"], thanks: "we_look" },
  problem: { watch: null, report: ["busy", "issue_link-problem"], thanks: "we_look" },
};

const share =
  (
    what: What,
    state: State,
    override: Partial<{ watch: WatchInfo; thanks: "tested_next" | "we_look"; report: [string, string]; reading: boolean }> = {},
  ) =>
  (lang: Lang): ScreenModel => {
    const source = { ...SOURCES[what], ...override };
    return {
      screen: "share",
      watch: source.watch,
      what,
      summary: shareSummary(lang, what, source.watch),
      reportText: reportText(...source.report),
      state,
      reading: source.reading ?? false,
      thanks: state === "opened" || state === "opened_paste" ? source.thanks : null,
    };
  };

export default {
  title: "Screens/Share",
  render: renderScreen,
  args: { onAction: fn(), model: share("result", "ready") },
} satisfies Meta<ScreenStoryArgs>;

export const ResultReady: ScreenStoryObj = { args: { model: share("result", "ready") } };
export const ResultOpened: ScreenStoryObj = { args: { model: share("result", "opened") } };
export const ResultOpenedPaste: ScreenStoryObj = { args: { model: share("result", "opened_paste") } };
export const ResultFailed: ScreenStoryObj = { args: { model: share("result", "failed") } };
/** Lily 2 Active's first confirmation: the next release lists it as confirmed. */
export const ResultOpenedFirstConfirmation: ScreenStoryObj = {
  args: {
    model: share("result", "opened", {
      watch: WATCHES.lily2Active,
      thanks: "tested_next",
      report: ["lily2active", "issue_link-georgian"],
    }),
  },
};
/** Venu 3 after a restart the app didn't verify: the watch is read again before the report (§4). */
export const ResultReading: ScreenStoryObj = {
  args: {
    model: share("result", "ready", {
      watch: WATCHES.venu3,
      reading: true,
      report: ["venu3-waiting", "issue_link-georgian"],
    }),
  },
};

export const FontListReady: ScreenStoryObj = { args: { model: share("font_list", "ready") } };
export const FontListOpened: ScreenStoryObj = { args: { model: share("font_list", "opened") } };
export const FontListOpenedPaste: ScreenStoryObj = { args: { model: share("font_list", "opened_paste") } };
export const FontListFailed: ScreenStoryObj = { args: { model: share("font_list", "failed") } };

export const ProblemReady: ScreenStoryObj = { args: { model: share("problem", "ready") } };
export const ProblemOpened: ScreenStoryObj = { args: { model: share("problem", "opened") } };
export const ProblemOpenedPaste: ScreenStoryObj = { args: { model: share("problem", "opened_paste") } };
export const ProblemFailed: ScreenStoryObj = { args: { model: share("problem", "failed") } };
