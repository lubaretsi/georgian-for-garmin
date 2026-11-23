import type { ReactNode } from "react";

import type { Lang, Platform } from "../../../core/backend";
import { cn } from "../lib/cn";

export type Theme = "light" | "dark";

/** macOS: the traffic lights sit over the app's own toolbar (a transparent, overlay title bar). */
export const MAC_INSET = 80;

/**
 * A stand-in for the app window as the OS draws it, at the app's inner size.
 * - macOS: an overlay title bar (Tauri `titleBarStyle: "Overlay"`, title hidden): the app's
 *   toolbar is the title bar, with the traffic lights drawn over its left end.
 * - Windows: the Windows 11 caption above the app (icon, title, minimise, maximise, close).
 * - Linux: a GNOME header bar with the title centred and a round close button.
 * The chrome is decoration (aria-hidden); the app inside is what the stories show.
 */
export function WindowFrame(props: {
  platform: Platform;
  theme: Theme;
  lang: Lang;
  width: number;
  height: number;
  title: string;
  children: ReactNode;
}) {
  const { platform } = props;
  return (
    <div
      className={cn(
        "design-b nb:flex nb:flex-col nb:overflow-hidden nb:bg-canvas nb:shadow-window",
        platform === "windows" ? "nb:rounded-[8px]" : "nb:rounded-[12px]",
      )}
      data-theme={props.theme}
      data-platform={platform}
      lang={props.lang}
      style={{ width: props.width }}
    >
      {platform === "windows" ? <WindowsCaption title={props.title} /> : null}
      {platform === "linux" ? <GnomeHeader title={props.title} /> : null}
      <div className="nb:relative" style={{ height: props.height }}>
        {props.children}
        {platform === "macos" ? <TrafficLights /> : null}
      </div>
    </div>
  );
}

function TrafficLights() {
  const light = (fill: string) => (
    <span
      className="nb:size-3 nb:rounded-full nb:shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.18)]"
      style={{ background: fill }}
    />
  );
  return (
    <div aria-hidden="true" className="nb:absolute nb:top-[20px] nb:left-5 nb:z-30 nb:flex nb:gap-2">
      {light("#ff5f57")}
      {light("#febc2e")}
      {light("#28c840")}
    </div>
  );
}

function WindowsCaption({ title }: { title: string }) {
  return (
    <div
      aria-hidden="true"
      className="nb:flex nb:h-8 nb:shrink-0 nb:items-center nb:border-b nb:border-sep nb:bg-canvas nb:pl-3 nb:text-label nb:text-fg"
      style={{ fontFamily: '"Segoe UI Variable Text", "Segoe UI", var(--b-font)' }}
    >
      <AppIcon />
      <span className="nb:ml-2.5">{title}</span>
      <span className="nb:ml-auto nb:flex nb:h-full">
        {["M3 8h10", "M3.5 3.5h9v9h-9z", "M3.5 3.5l9 9M12.5 3.5l-9 9"].map((d) => (
          <span key={d} className="nb:flex nb:h-full nb:w-[46px] nb:items-center nb:justify-center">
            <svg width="16" height="16" viewBox="0 0 16 16">
              <path d={d} fill="none" stroke="currentColor" strokeWidth="1" />
            </svg>
          </span>
        ))}
      </span>
    </div>
  );
}

function GnomeHeader({ title }: { title: string }) {
  return (
    <div
      aria-hidden="true"
      className="nb:relative nb:flex nb:h-[46px] nb:shrink-0 nb:items-center nb:justify-center nb:border-b nb:border-sep nb:bg-canvas nb:text-row nb:font-bold nb:text-fg"
      style={{ fontFamily: 'Cantarell, "Adwaita Sans", var(--b-font)' }}
    >
      {title}
      <span className="nb:absolute nb:right-2.5 nb:flex nb:size-6 nb:items-center nb:justify-center nb:rounded-full nb:bg-fill">
        <svg width="12" height="12" viewBox="0 0 16 16">
          <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      </span>
    </div>
  );
}

/** The app icon, small: the red tile with a white "ქ". */
export function AppIcon({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <rect width="16" height="16" rx="3.6" fill="#e0344f" />
      <text x="8" y="12" textAnchor="middle" fontSize="11" fontWeight="700" fill="#fffbfb" style={{ fontFamily: "var(--b-font)" }}>
        ქ
      </text>
    </svg>
  );
}
