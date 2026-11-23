import type { ReactNode } from "react";

import type { Lang } from "../core/backend";

export type Theme = "light" | "dark";

/** Window sizes for Storybook (§8): the app window's inner size. */
export const WINDOW_SIZES = {
  min: { width: 420, height: 560 },
  default: { width: 480, height: 720 },
  large: { width: 640, height: 800 },
  short: { width: 420, height: 520 },
} as const;
export type WindowSize = keyof typeof WINDOW_SIZES;

/** A fixed-size stand-in for the app window, so stories render at the window's size. */
export function WindowFrame(props: {
  width: number;
  height: number;
  theme: Theme;
  lang: Lang;
  children: ReactNode;
}) {
  return (
    <div
      className="window-frame"
      data-theme={props.theme}
      lang={props.lang}
      style={{ width: props.width, height: props.height }}
    >
      {props.children}
    </div>
  );
}
