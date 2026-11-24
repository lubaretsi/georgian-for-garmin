import type { ReactNode } from "react";

import type { Lang, Platform } from "../core/backend";
import { AppIcon } from "./art/AppIcon";
import { cn } from "./cn";

export type Theme = "light" | "dark";

/** Window sizes for Storybook (§8): the app window's inner size. */
export const WINDOW_SIZES = {
  min: { width: 420, height: 560 },
  default: { width: 480, height: 720 },
  large: { width: 640, height: 800 },
  short: { width: 420, height: 520 },
} as const;
export type WindowSize = keyof typeof WINDOW_SIZES;

/** On macOS the traffic lights sit over the left end of the app's toolbar (§10). */
export const MAC_TOOLBAR_INSET = 80;

/**
 * A stand-in for the app window as the OS draws it, with the app at `width`×`height` inside:
 * - macOS: the overlay title bar (Tauri `titleBarStyle: "Overlay"`, title hidden); the app's
 *   toolbar is the title bar and the traffic lights are drawn over its left end;
 * - Windows: the Windows 11 caption (icon, title, minimise, maximise, close) above the app;
 * - Linux: a GNOME header bar with the title centred and a round close button.
 * The chrome is decoration (aria-hidden).
 */
export function WindowFrame(props: {
  width: number;
  height: number;
  theme: Theme;
  lang: Lang;
  platform?: Platform;
  children: ReactNode;
}) {
  const platform = props.platform ?? "macos";
  return (
    <div
      className={cn(
        "window-frame flex flex-col overflow-hidden bg-canvas shadow-window",
        platform === "windows" ? "rounded-[8px]" : "rounded-[12px]",
      )}
      data-theme={props.theme}
      data-platform={platform}
      lang={props.lang}
      style={{ width: props.width }}
    >
      {platform === "windows" ? <WindowsCaption /> : null}
      {platform === "linux" ? <GnomeHeader /> : null}
      <div className="relative" style={{ height: props.height }}>
        {props.children}
        {platform === "macos" ? <TrafficLights /> : null}
      </div>
    </div>
  );
}

const TITLE = "Georgian for Garmin";

function TrafficLights() {
  return (
    <div aria-hidden="true" className="pointer-events-none absolute top-5 left-5 z-30 flex gap-2">
      {["#ff5f57", "#febc2e", "#28c840"].map((fill) => (
        <span key={fill} className="size-3 rounded-full shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.18)]" style={{ background: fill }} />
      ))}
    </div>
  );
}

function WindowsCaption() {
  return (
    <div
      aria-hidden="true"
      className="flex h-8 shrink-0 items-center border-b border-sep bg-canvas pl-3 text-label text-fg"
      style={{ fontFamily: '"Segoe UI Variable Text", "Segoe UI", var(--font)' }}
    >
      <AppIcon size={16} />
      <span className="ml-2.5">{TITLE}</span>
      <span className="ml-auto flex h-full">
        {["M3 8h10", "M3.5 3.5h9v9h-9z", "M3.5 3.5l9 9M12.5 3.5l-9 9"].map((d) => (
          <span key={d} className="flex h-full w-[46px] items-center justify-center">
            <svg width="16" height="16" viewBox="0 0 16 16">
              <path d={d} fill="none" stroke="currentColor" strokeWidth="1" />
            </svg>
          </span>
        ))}
      </span>
    </div>
  );
}

function GnomeHeader() {
  return (
    <div
      aria-hidden="true"
      className="relative flex h-[46px] shrink-0 items-center justify-center border-b border-sep bg-canvas text-row font-bold text-fg"
      style={{ fontFamily: 'Cantarell, "Adwaita Sans", var(--font)' }}
    >
      {TITLE}
      <span className="absolute right-2.5 flex size-6 items-center justify-center rounded-full bg-fill">
        <svg width="12" height="12" viewBox="0 0 16 16">
          <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      </span>
    </div>
  );
}
