// A flow story: the whole app (shell, flow reducer and effects, MockBackend) in a window of the
// toolbar's size, with the watch simulator beside it.

import { useState } from "react";
import { within } from "storybook/test";

import { type Copy, copyFor, fmt } from "../../copy";
import { FlowApp } from "../../flow/FlowApp";
import { MockBackend, type MockOptions } from "../../mock/MockBackend";
import { Simulator } from "../../mock/Simulator";
import { WindowFrame } from "../../ui/WindowFrame";
import { type StoryGlobals, storyGlobals } from "../../../.storybook/globals";

/** The story tests run the flow and the watch this much faster (vite.config.ts). */
export const SPEED = Number(import.meta.env.VITE_FLOW_SPEED ?? 1);
/** How long a play function waits for the next screen. */
export const STEP_MS = 8000;
const VERSION = "0.3.4";

export function FlowStory(props: { options: MockOptions; globals: Partial<StoryGlobals> }) {
  const g = storyGlobals(props.globals);
  const [backend] = useState(() => new MockBackend({ platform: g.platform, speed: SPEED, ...props.options }));
  return (
    <div style={{ display: "flex", gap: 24, alignItems: "flex-start", flexWrap: "wrap" }}>
      <WindowFrame width={g.width} height={g.height} theme={g.theme} lang={g.lang} platform={g.platform}>
        <FlowApp backend={backend} lang={g.lang} platform={g.platform} version={VERSION} speed={SPEED} />
      </WindowFrame>
      <Simulator backend={backend} />
    </div>
  );
}

/** A rendered flow story's window and simulator, and the copy it uses. */
export type FlowParts = {
  app: ReturnType<typeof within>;
  sim: ReturnType<typeof within>;
  copy: Copy;
  fmt: typeof fmt;
  /** Waits for the screen whose heading is `title`. */
  screen: (title: string) => Promise<HTMLElement>;
  /** The app's button called `name`, once it shows. */
  button: (name: string) => Promise<HTMLElement>;
};

export function parts(canvasElement: HTMLElement, globals: Partial<StoryGlobals>): FlowParts {
  const frame = canvasElement.querySelector<HTMLElement>(".window-frame");
  const panel = canvasElement.querySelector<HTMLElement>('aside[aria-label="Watch simulator"]');
  if (!frame || !panel) throw new Error("flow story: no window or simulator");
  const copy: Copy = copyFor(storyGlobals(globals).lang);
  // The whole document, not just the window: sheets and dialogs render in portals outside it.
  // The simulator's controls and headings are named so they never match the app's copy.
  const app = within(frame.ownerDocument.body);
  return {
    app,
    sim: within(panel),
    copy,
    fmt,
    /** Waits for the screen whose heading is `title`. */
    screen: (title: string) => app.findByRole("heading", { name: title }, { timeout: STEP_MS }),
    /** The app's button called `name`. */
    button: (name: string) => app.findByRole("button", { name }, { timeout: STEP_MS }),
  };
}
