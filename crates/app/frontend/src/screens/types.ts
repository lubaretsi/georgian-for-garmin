import type { ReactNode } from "react";

import type { Lang, Platform } from "../core/backend";
import type { Copy } from "../copy";
import type { ActionId, ActionPayload } from "../model/screens";
import type { StepId } from "../ui/StepIndicator";

export type Emit = (id: ActionId, payload?: ActionPayload) => void;

/** What a screen gets: the copy for the chosen language, the platform and the action sink. */
export type ScreenContext = { copy: Copy; lang: Lang; platform: Platform; emit: Emit };

/** The id of every screen's heading, for controls the heading names (the answer group). */
export const TITLE_ID = "screen-title";

/**
 * A footer button. It either calls `onPress` (usually emitting an ActionId) or, with
 * `copyText`, copies that text and says so on the button for a moment.
 */
export type FooterAction = {
  label: string;
  onPress?: () => void;
  copyText?: string;
  disabled?: boolean;
};

/**
 * What a screen draws. The shell places the parts: the step indicator in the toolbar, the
 * hero picture, the heading (which gets focus on every screen change), the lead, the content
 * (which scrolls), and the footer: the status on the left, the actions on the right.
 * Screens are pure: the same model and context give the same view.
 */
export type ScreenView = {
  step: StepId | null;
  /** Every step shows as done (Georgian works). */
  allDone?: boolean;
  title: string;
  hero?: ReactNode;
  /** A small badge above the heading, e.g. "Attempt 2". */
  badge?: string;
  lead?: string;
  /** The footer's status, also announced in the polite live region when it changes. */
  status?: { text: string; busy: boolean };
  content?: ReactNode;
  primary?: FooterAction;
  secondary?: FooterAction;
  /** A Back button at the toolbar's left, for screens off the journey (diagnostics). */
  back?: FooterAction;
};
