import type { ReactNode } from "react";

import type { Lang, Platform } from "../core/backend";
import type { Copy } from "../copy";
import type { ActionId, ActionPayload, WatchInfo } from "../model/screens";
import type { FooterAction } from "../ui/ActionBar";
import type { IconName } from "../ui/icons";
import type { StepId } from "../ui/StepIndicator";

export type Emit = (id: ActionId, payload?: ActionPayload) => void;

/** What a screen gets: the copy for the chosen language, the platform and the action sink. */
export type ScreenContext = { copy: Copy; lang: Lang; platform: Platform; emit: Emit };

/**
 * What a screen draws. The shell places the parts: the step indicator, the heading (which
 * gets focus on every screen change), the content (which scrolls) and the footer actions.
 * Screens are pure: the same model and context give the same view.
 */
export type ScreenView = {
  step: StepId | null;
  title: string;
  /** A small line above the heading, e.g. "Attempt 2". */
  badge?: string;
  illustration?: { icon: IconName; tone?: "accent" | "success" | "muted" };
  /** The watch the screen is about, shown under the heading. */
  watch?: WatchInfo;
  /** Status announced in the polite live region when it changes. */
  status?: string;
  content: ReactNode;
  primary?: FooterAction;
  secondary?: FooterAction;
};
