import type { Lang, Platform } from "../src/core/backend";
import { type Theme, WINDOW_SIZES, type WindowSize } from "../src/ui/WindowFrame";

/** The toolbar's names for the window sizes (contract §8). */
export const windowSizeTitles: Record<WindowSize, string> = {
  min: "Minimum 420×560",
  default: "Default 480×720",
  large: "Large 640×800",
  short: "Short screen 420×520",
};

/** The toolbar's globals, as stories read them from `context.globals`. */
export type StoryGlobals = { lang: Lang; platform: Platform; theme: Theme; size: WindowSize };

export const initialGlobals: StoryGlobals = {
  lang: "ka",
  platform: "macos",
  theme: "light",
  size: "default",
};

/** Reads the toolbar globals with their defaults filled in. */
export function storyGlobals(globals: Partial<StoryGlobals>): StoryGlobals & { width: number; height: number } {
  const all = { ...initialGlobals, ...globals };
  return { ...all, ...WINDOW_SIZES[all.size] };
}
