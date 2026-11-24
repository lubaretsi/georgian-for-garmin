import type { Meta } from "@storybook/react-vite";
import { fn } from "storybook/test";

import type { Lang } from "../../core/backend";
import type { ScreenModel } from "../../model/screens";
import { diagnosticsSample, type DiagnosticsCase } from "./data";
import { renderScreen, type ScreenStoryObj } from "./render";
import type { ScreenStoryArgs } from "./ScreenStory";

type State = Extract<ScreenModel, { screen: "diagnostics" }>["state"];

const SAVED_PATH = "/Users/nino/Downloads/georgian-for-garmin-diagnostics-2026-11-24-0314.txt";

/** More › Diagnostic info (§11), from the summary and text the flow builds from `diagnose`. */
const diagnostics =
  (state: State, found: DiagnosticsCase = "watch") =>
  (lang: Lang): ScreenModel => {
    const sample = diagnosticsSample(lang, found);
    return {
      screen: "diagnostics",
      state,
      // Before `diagnose` answers the flow knows only the app's version.
      summary: state === "collecting" ? sample.summary.slice(0, 1) : sample.summary,
      text: state === "collecting" ? "" : sample.text,
      savedPath: state === "saved" ? SAVED_PATH : null,
    };
  };

export default {
  title: "Screens/Diagnostics",
  render: renderScreen,
  args: { onAction: fn(), model: diagnostics("ready") },
} satisfies Meta<ScreenStoryArgs>;

export const Collecting: ScreenStoryObj = { args: { model: diagnostics("collecting") } };
export const ReadyNoWatch: ScreenStoryObj = { args: { model: diagnostics("ready", "no_watch") } };
export const ReadyWatchFound: ScreenStoryObj = { args: { model: diagnostics("ready", "watch") } };
/** Garmin Express holds the watch: the case diagnostic info exists for. */
export const ReadyBusy: ScreenStoryObj = { args: { model: diagnostics("ready", "busy") } };
export const Saved: ScreenStoryObj = { args: { model: diagnostics("saved", "busy") } };
export const Copied: ScreenStoryObj = { args: { model: diagnostics("copied", "busy") } };
export const Failed: ScreenStoryObj = { args: { model: diagnostics("failed", "busy") } };
