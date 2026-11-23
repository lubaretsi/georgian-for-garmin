import { type ClassValue, clsx } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/** Joins classes and resolves Tailwind conflicts for this direction's `wa:` prefix. */
const merge = extendTailwindMerge({ prefix: "wa" });

export function cn(...inputs: ClassValue[]): string {
  return merge(clsx(inputs));
}
