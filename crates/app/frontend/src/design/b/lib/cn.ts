import { type ClassValue, clsx } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/** tailwind-merge that knows this direction's `nb:` prefix and its own text sizes. */
const merge = extendTailwindMerge({
  prefix: "nb",
  extend: {
    theme: {
      text: ["title", "lead", "row", "label"],
      color: [
        "canvas", "group", "group-hover", "group-active", "fill", "fill-strong", "fg", "fg-2", "sep",
        "control", "control-bg", "accent", "accent-hover", "accent-active", "on-accent", "accent-ink",
        "indicator", "accent-soft", "success", "success-ink", "success-soft", "caution-soft",
        "caution-ink", "tile", "toolbar",
      ],
      radius: ["group", "control", "tile"],
      shadow: ["group", "control", "thumb", "window"],
    },
  },
});

export function cn(...inputs: ClassValue[]): string {
  return merge(clsx(inputs));
}
