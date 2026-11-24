import { describe, expect, it } from "vitest";

import type { Platform } from "../../core/backend";
import { covers, REQUIRED } from "./coverage";
import { resolveModel } from "./data";
import type { ScreenStoryArgs } from "./ScreenStory";

/** A CSF module: the default export (meta, with a title) and the stories, all with args. */
type StoryModule = Record<string, { title?: string; args?: Partial<ScreenStoryArgs> }>;

const modules = import.meta.glob<StoryModule>("./*.stories.tsx", { eager: true });

/** Every Screens/… story with its effective args. */
const stories = Object.entries(modules).flatMap(([file, mod]) => {
  const meta = mod.default;
  if (!meta?.title) throw new Error(`${file} has no meta title`);
  return Object.entries(mod)
    .filter(([name]) => name !== "default")
    .map(([name, story]) => {
      const args = { ...meta.args, ...story.args };
      if (!args.model) throw new Error(`${file} ${name} has no model`);
      return { id: `${meta.title}/${name}`, model: args.model, platform: args.platform };
    });
});
describe("Screens stories (§8)", () => {
  it("cover every item of the coverage list", () => {
    const covered = new Set(
      stories.flatMap((s) => {
        const platforms: Platform[] = s.platform ? [s.platform] : ["macos"];
        return platforms.flatMap((p) => covers(resolveModel(s.model, "en"), p));
      }),
    );
    const missing = REQUIRED.filter((item) => !covered.has(item));
    expect(missing, "coverage items with no story").toEqual([]);
  });

  it("build in both languages", () => {
    for (const s of stories) {
      for (const lang of ["ka", "en"] as const) expect(() => resolveModel(s.model, lang), s.id).not.toThrow();
    }
  });
});
