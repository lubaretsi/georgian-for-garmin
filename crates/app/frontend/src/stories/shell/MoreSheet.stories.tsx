import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";

import { storyGlobals } from "../../../.storybook/globals";
import { copyFor } from "../../copy";
import { REPO_URL } from "../../copy/shared";
import { WATCHES } from "../screens/data";
import { renderScreen } from "../screens/render";
import type { ScreenStoryArgs } from "../screens/ScreenStory";

/** More, opened over a Venu 2's install screen: what the app sees, help and About. */
export default {
  title: "Shell/More",
  render: renderScreen,
  args: {
    onAction: fn(),
    restorable: true,
    model: {
      screen: "watch",
      watch: WATCHES.venu2,
      verdict: { kind: "install", confidence: "confirmed" },
      issue: null,
      reportWanted: false,
      sharedBefore: false,
    },
  },
} satisfies Meta<ScreenStoryArgs>;

export const Open: StoryObj<ScreenStoryArgs> = {
  play: async ({ canvasElement, globals }) => {
    const copy = copyFor(storyGlobals(globals).lang);
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: copy.header.more }));
    await canvas.findByRole("dialog", { name: copy.more.title });
    // The repository's URL wraps at the space before it, onto a line of its own, rather than
    // breaking after "https://github.co" (break-all did that).
    const url = await canvas.findByText(REPO_URL);
    const paragraph = url.closest("p");
    if (!paragraph) throw new Error("the URL is not in a paragraph");
    const start = url.getClientRects()[0];
    expect(start).toBeDefined();
    expect(Math.abs((start?.left ?? 0) - paragraph.getBoundingClientRect().left)).toBeLessThan(1);
  },
};
