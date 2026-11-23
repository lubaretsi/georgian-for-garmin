import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
import { useGlobals } from "storybook/preview-api";
import { fn } from "storybook/test";

import { storyGlobals } from "../../../.storybook/globals";
import type { Lang } from "../../core/backend";
import { DesignB } from "../../design/b/DesignB";
import type { ActionId, ActionPayload, ScreenModel } from "../../model/screens";
import { moreFor, WATCHES } from "../screens/data";

type Args = { model: ScreenModel; restorable?: boolean; onAction: (id: ActionId, payload?: ActionPayload) => void };

/**
 * Direction B, "Native & precise": the five checkpoint screens in their window, at the
 * toolbar's size, theme, language and platform. Answers on the check screen apply locally.
 */
function Render(args: Args) {
  const [globals, updateGlobals] = useGlobals();
  const g = storyGlobals(globals);
  const [model, setModel] = useState(args.model);
  useEffect(() => setModel(args.model), [args.model]);
  const watch = "watch" in model ? model.watch : null;
  return (
    <DesignB
      model={model}
      more={moreFor(g.lang, watch, args.restorable ?? false)}
      lang={g.lang}
      platform={g.platform}
      theme={g.theme}
      width={g.width}
      height={g.height}
      onAction={(id, payload) => {
        if (payload?.action === "answer" && model.screen === "check") setModel({ ...model, answers: payload.answers });
        args.onAction(id, payload);
      }}
      onLanguage={(lang: Lang) => updateGlobals({ lang })}
    />
  );
}

const lily = WATCHES.lily2Active;

export default {
  title: "Design/B Native & precise",
  render: Render,
  parameters: { frame: false, layout: "centered" },
  args: { onAction: fn(), model: { screen: "connect", mode: "searching", showHelp: true } },
} satisfies Meta<Args>;

type Story = StoryObj<Args>;

export const ConnectSearching: Story = {
  name: "1 Connect, searching with help",
  args: { model: { screen: "connect", mode: "searching", showHelp: true } },
};

export const WatchInstall: Story = {
  name: "2 Watch, install (unconfirmed)",
  args: {
    model: { screen: "watch", watch: lily, verdict: { kind: "install", confidence: "unconfirmed" }, issue: null, reportWanted: true, sharedBefore: false },
  },
};

export const Restart: Story = {
  name: "3 Restart the watch",
  args: {
    model: { screen: "restart", op: "install", watch: lily, phase: "restart", attempt: 1, verifiable: true },
    restorable: true,
  },
};

export const CheckAnswer: Story = {
  name: "4 Check, answer",
  args: {
    model: {
      screen: "check",
      watch: lily,
      step: "answer",
      answers: { result: "georgian", views: ["popup"], phone: "android" },
      phone: null,
      markPictures: [],
    },
  },
};

export const OutcomeWorksFirst: Story = {
  name: "5 Outcome, works (first)",
  args: { model: { screen: "outcome", watch: lily, kind: "works_first", restarts: 1, sharedBefore: false }, restorable: true },
};
