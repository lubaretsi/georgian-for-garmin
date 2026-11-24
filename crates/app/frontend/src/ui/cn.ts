import { type ClassValue, clsx } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/** tailwind-merge that knows the app's own text sizes, colours, radii and shadows (global.css). */
const merge = extendTailwindMerge({
  extend: {
    theme: {
      text: ["title", "lead", "row", "label"],
      color: [
        "canvas", "group", "group-hover", "group-active", "fill", "fill-strong", "fg", "fg-2", "sep",
        "control", "control-bg", "accent", "accent-hover", "accent-active", "on-accent", "accent-ink",
        "indicator", "accent-soft", "success", "success-ink", "success-soft", "caution-ink", "caution-soft",
        "danger-ink", "danger-soft",
      ],
      radius: ["group", "control", "tile"],
      shadow: ["group", "control", "thumb", "window"],
    },
  },
});

export function cn(...inputs: ClassValue[]): string {
  return merge(clsx(inputs));
}
