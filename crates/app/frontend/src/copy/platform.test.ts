// Platform variants (`{ macos, windows, linux }`, §7): each shows only its own platform's steps.

import { describe, expect, it } from "vitest";

import type { Platform } from "../core/backend";
import { en } from "./en";
import { ka } from "./ka";

const PLATFORMS: readonly Platform[] = ["macos", "windows", "linux"];

/** Words of one platform's steps, which no other platform's words may use. */
const PLATFORM_WORDS: Record<Platform, string[]> = {
  macos: ["Activity Monitor", "menu bar", "მენიუს ზოლ", "Mac", "Terminal", "Finder"],
  windows: ["Task Manager", "notification area", "შეტყობინებების არე", "დროის გვერდით", "Windows", "Explorer"],
  linux: ["file manager", "ფაილების მენეჯერ", "Linux", "KDE"],
};

/** Every string under `value`, with its dotted path. */
function strings(value: unknown, path: string, out: [string, string][] = []): [string, string][] {
  if (typeof value === "string") out.push([path, value]);
  else if (value && typeof value === "object") {
    for (const [key, inner] of Object.entries(value)) strings(inner, `${path}.${key}`, out);
  }
  return out;
}

/** Each `{ macos, windows, linux }` table in `value`, with its path. */
function variants(value: unknown, path = ""): [string, Record<Platform, unknown>][] {
  if (!value || typeof value !== "object" || Array.isArray(value)) return [];
  const keys = Object.keys(value);
  if (keys.some((k) => (PLATFORMS as string[]).includes(k))) {
    expect(keys.sort(), `${path}: a platform table has exactly the three platforms`).toEqual([...PLATFORMS].sort());
    return [[path, value as Record<Platform, unknown>]];
  }
  return Object.entries(value).flatMap(([key, inner]) => variants(inner, path ? `${path}.${key}` : key));
}

describe("platform variants", () => {
  for (const [lang, catalog] of [["ka", ka], ["en", en]] as const) {
    const tables = variants(catalog);

    it(`${lang}: the busy steps and the connect help have one per platform`, () => {
      const paths = tables.map(([path]) => path);
      expect(paths).toContain("connect.help.express");
      expect(paths.filter((p) => p.startsWith("connect.busy.steps.")).length).toBeGreaterThanOrEqual(6);
    });

    it(`${lang}: no platform shows another platform's steps`, () => {
      for (const [path, table] of tables) {
        for (const platform of PLATFORMS) {
          for (const [where, text] of strings(table[platform], `${path}.${platform}`)) {
            for (const other of PLATFORMS.filter((p) => p !== platform)) {
              const found = PLATFORM_WORDS[other].filter((word) => text.includes(word));
              expect(found, `${lang} ${where} has ${other}'s words`).toEqual([]);
            }
            // Garmin Express doesn't run on Linux; only its own holder's steps may name it.
            if (platform === "linux" && !path.endsWith(".garmin_express")) {
              expect(text, `${lang} ${where}`).not.toMatch(/Garmin Express/);
            }
          }
        }
      }
    });

    it(`${lang}: the busy steps ask for no Terminal or administrator commands`, () => {
      for (const [where, text] of strings(catalog.connect.busy, `${lang}.connect.busy`)) {
        for (const word of ["pkill", "Terminal", "sudo"]) expect(text, where).not.toContain(word);
      }
    });
  }
});
