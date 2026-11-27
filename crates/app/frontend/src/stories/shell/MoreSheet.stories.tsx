import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, waitFor, within } from "storybook/test";

import { storyGlobals } from "../../../.storybook/globals";
import type { Lang } from "../../core/backend";
import { copyFor, fmt } from "../../copy";
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

/** Opens More and checks About's URL starts its own line (break-all split it mid-word). */
async function openMore(canvasElement: HTMLElement, lang: Lang) {
  const copy = copyFor(lang);
  const canvas = within(canvasElement);
  await userEvent.click(await canvas.findByRole("button", { name: copy.header.more }));
  const dialog = await canvas.findByRole("dialog", { name: copy.more.title });
  const url = await canvas.findByText(REPO_URL);
  const paragraph = url.closest("p");
  if (!paragraph) throw new Error("the URL is not in a paragraph");
  const start = url.getClientRects()[0];
  expect(start).toBeDefined();
  expect(Math.abs((start?.left ?? 0) - paragraph.getBoundingClientRect().left)).toBeLessThan(1);
  return dialog;
}

/** The scroll areas' `scrollbar-width`: the screen's and More's. */
function scrollbarWidths(canvasElement: HTMLElement, dialog: HTMLElement): string[] {
  const areas = [canvasElement.querySelector("main"), dialog.querySelector(".overflow-y-auto")];
  return areas.map((area) => {
    if (!area) throw new Error("a scroll area is missing");
    return getComputedStyle(area).getPropertyValue("scrollbar-width");
  });
}

export const Open: StoryObj<ScreenStoryArgs> = {
  args: { platform: "macos" },
  play: async ({ canvasElement, globals }) => {
    const dialog = await openMore(canvasElement, storyGlobals(globals).lang);
    // macOS overlays its scrollbars; the app leaves them alone.
    if (CSS.supports("scrollbar-width", "thin")) expect(scrollbarWidths(canvasElement, dialog)).toEqual(["auto", "auto"]);
  },
};

export const OpenOnWindows: StoryObj<ScreenStoryArgs> = {
  args: { platform: "windows" },
  play: async ({ canvasElement, globals }) => {
    const dialog = await openMore(canvasElement, storyGlobals(globals).lang);
    // WebView2's classic scrollbar pushes the content off center; the thin one takes less.
    if (CSS.supports("scrollbar-width", "thin")) expect(scrollbarWidths(canvasElement, dialog)).toEqual(["thin", "thin"]);
  },
};

/** The shortest window: More scrolls, and no group cuts off its rows. */
export const OpenInAShortWindow: StoryObj<ScreenStoryArgs> = {
  globals: { size: "short" },
  play: async ({ canvasElement, globals }) => {
    const dialog = await openMore(canvasElement, storyGlobals(globals).lang);
    const list = dialog.querySelector(".overflow-y-auto");
    if (!list) throw new Error("More's list is missing");
    expect(list.scrollHeight).toBeGreaterThan(list.clientHeight);
    for (const section of Array.from(list.children)) {
      expect(section.scrollHeight - section.clientHeight).toBeLessThanOrEqual(1);
    }
  },
};

/**
 * A newer version is published (§12): the quiet note under the screen and in More › About, each
 * emitting `open_update`, which opens the release page.
 */
async function pressUpdateNotes(canvasElement: HTMLElement, lang: Lang, onAction: ScreenStoryArgs["onAction"]) {
  const copy = copyFor(lang);
  const canvas = within(canvasElement);
  const name = fmt(copy.update.available, { version: "0.5.0" });
  const note = await canvas.findByRole("button", { name });
  expect(note).toHaveAccessibleDescription(copy.update.download);
  // Never one of the footer's actions.
  expect(note.closest("footer")).toBeNull();
  await userEvent.click(note);
  expect(onAction).toHaveBeenLastCalledWith("open_update", undefined);
  const dialog = await openMore(canvasElement, lang);
  await userEvent.click(within(dialog).getByRole("button", { name }));
  expect(onAction).toHaveBeenLastCalledWith("open_update", undefined);
  // Pressing it closes More, as every command in the sheet does.
  await waitFor(() => expect(canvas.queryByRole("dialog")).toBeNull());
}

export const UpdateAvailableEn: StoryObj<ScreenStoryArgs> = {
  args: { update: "0.5.0", onAction: fn() },
  globals: { lang: "en" },
  play: ({ canvasElement, args }) => pressUpdateNotes(canvasElement, "en", args.onAction),
};

export const UpdateAvailableKa: StoryObj<ScreenStoryArgs> = {
  args: { update: "0.5.0", onAction: fn() },
  globals: { lang: "ka" },
  play: ({ canvasElement, args }) => pressUpdateNotes(canvasElement, "ka", args.onAction),
};
