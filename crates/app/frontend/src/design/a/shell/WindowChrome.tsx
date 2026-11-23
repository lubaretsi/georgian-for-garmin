import { Minus, Square, X } from "lucide-react";
import type { ReactNode } from "react";

import type { Platform } from "@/core/backend";

import { AppIcon } from "../art/app-icon";
import { cn } from "../ui/cn";

/**
 * A stand-in for the native window around the app, for review only: macOS title bar with
 * traffic lights, Windows 11 caption buttons, or a GNOME header bar. The content area is
 * exactly `width`×`height`, the app window's inner size.
 */
export function WindowChrome(props: { platform: Platform; title: string; width: number; height: number; dark: boolean; children: ReactNode }) {
  const { platform, dark } = props;
  const bar = {
    macos: cn("wa:h-7 wa:justify-center", dark ? "wa:bg-[#2b2826] wa:text-[#d8d2cc]" : "wa:bg-[#ebe6e0] wa:text-[#4a4440]"),
    windows: cn("wa:h-8 wa:justify-start wa:pl-3", dark ? "wa:bg-[#202020] wa:text-[#e6e6e6]" : "wa:bg-[#f3f3f3] wa:text-[#1a1a1a]"),
    linux: cn("wa:h-9 wa:justify-center", dark ? "wa:bg-[#303030] wa:text-[#eeeeee]" : "wa:bg-[#ebebeb] wa:text-[#2e2e2e]"),
  }[platform];
  return (
    <div
      data-window
      className={cn(
        "wa:overflow-hidden wa:shadow-[0_0_0_0.5px_rgb(0_0_0/0.25),0_24px_60px_-18px_rgb(40_20_10/0.45),0_8px_20px_-10px_rgb(40_20_10/0.25)]",
        platform === "macos" ? "wa:rounded-[11px]" : platform === "windows" ? "wa:rounded-[8px]" : "wa:rounded-[12px]",
      )}
      style={{ width: props.width }}
    >
      <div
        aria-hidden
        className={cn(
          "wa:relative wa:flex wa:items-center wa:border-b wa:text-[13px] wa:select-none",
          dark ? "wa:border-black/50" : "wa:border-black/10",
          bar,
        )}
      >
        {platform === "macos" ? (
          <span className="wa:absolute wa:left-3 wa:flex wa:gap-2">
            {["#ff5f57", "#febc2e", "#28c840"].map((c) => (
              <span key={c} className="wa:size-3 wa:rounded-full wa:shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.18)]" style={{ background: c }} />
            ))}
          </span>
        ) : null}
        {platform === "windows" ? <AppIcon size={16} className="wa:mr-2" /> : null}
        <span className={cn(platform === "macos" ? "wa:font-semibold" : "wa:font-normal")} style={{ fontFamily: platform === "windows" ? "'Segoe UI Variable', 'Segoe UI', system-ui" : "system-ui" }}>
          {props.title}
        </span>
        {platform === "windows" ? (
          <span className="wa:absolute wa:inset-y-0 wa:right-0 wa:flex">
            {[Minus, Square, X].map((Icon, i) => (
              <span key={i} className="wa:grid wa:w-[46px] wa:place-items-center">
                <Icon className={i === 1 ? "wa:size-[10px]" : "wa:size-[14px]"} strokeWidth={1.25} />
              </span>
            ))}
          </span>
        ) : null}
        {platform === "linux" ? (
          <span className={cn("wa:absolute wa:right-2 wa:grid wa:size-6 wa:place-items-center wa:rounded-full", dark ? "wa:bg-white/10" : "wa:bg-black/[0.07]")}>
            <X className="wa:size-3.5" strokeWidth={2} />
          </span>
        ) : null}
      </div>
      <div style={{ width: props.width, height: props.height }}>{props.children}</div>
    </div>
  );
}
