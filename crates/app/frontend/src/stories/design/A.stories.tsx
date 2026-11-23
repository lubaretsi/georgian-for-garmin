import type { Meta, StoryObj } from "@storybook/react-vite";
import { useGlobals } from "storybook/preview-api";
import { fn } from "storybook/test";

import { storyGlobals } from "../../../.storybook/globals";
import type { Lang } from "../../core/backend";
import { Preview } from "../../design/a/Preview";
import type { ActionId, ActionPayload, ScreenModel } from "../../model/screens";
import { moreFor, phone, WATCHES } from "../screens/data";

type Args = {
  model: (lang: Lang) => ScreenModel;
  restorable?: boolean;
  onAction: (id: ActionId, payload?: ActionPayload) => void;
};

/**
 * Direction A, "Warm & illustrated": the five checkpoint screens in a native-looking window
 * at the toolbar's size, theme, language and platform.
 */
export default {
  title: "Design/A Warm & illustrated",
  parameters: { frame: false, layout: "fullscreen" },
  args: { onAction: fn() },
  render: function Render(args) {
    const [globals, updateGlobals] = useGlobals();
    const g = storyGlobals(globals);
    const model = args.model(g.lang);
    const watch = "watch" in model ? model.watch : null;
    return (
      <Preview
        model={model}
        more={moreFor(g.lang, watch, args.restorable ?? false)}
        lang={g.lang}
        platform={g.platform}
        theme={g.theme}
        width={g.width}
        height={g.height}
        onAction={args.onAction}
        onLanguage={(lang) => updateGlobals({ lang })}
      />
    );
  },
} satisfies Meta<Args>;

type Story = StoryObj<Args>;

export const Connect: Story = {
  name: "1 Connect, searching with help",
  args: { model: () => ({ screen: "connect", mode: "searching", showHelp: true }) },
};

export const Install: Story = {
  name: "2 Install, not yet confirmed",
  args: {
    model: () => ({
      screen: "watch",
      watch: WATCHES.lily2Active,
      verdict: { kind: "install", confidence: "unconfirmed" },
      issue: null,
      reportWanted: true,
      sharedBefore: false,
    }),
  },
};

export const Restart: Story = {
  name: "3 Restart, after install",
  args: {
    model: () => ({ screen: "restart", op: "install", watch: WATCHES.lily2Active, phase: "restart", attempt: 1, verifiable: true }),
  },
};

export const Check: Story = {
  name: "4 Check, answered",
  args: {
    model: (lang) => ({
      screen: "check",
      watch: WATCHES.lily2Active,
      step: "answer",
      answers: { result: "georgian", views: ["popup"], phone: "android" },
      phone: phone(lang),
      markPictures: [],
    }),
  },
};

export const Outcome: Story = {
  name: "5 Outcome, first to confirm",
  args: {
    model: () => ({ screen: "outcome", watch: WATCHES.lily2Active, kind: "works_first", restarts: 1, sharedBefore: false }),
    restorable: true,
  },
};
